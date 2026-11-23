//! Chapter 10 slots persist ground-space game state, not either renderer's entities.
mod storage;
mod ui;

use bevy::prelude::*;

use crate::{
    characters::{Player, Roster, RosterHandle},
    combat::Health,
    enemy::Enemy,
    game_state::GameState,
    inventory::{Inventory, Pickable},
    map::{MapSeed, collision::CollisionMap},
};

use storage::{EnemySave, HealthSave, PickupSave, PlayerSave, Snapshot};
pub(crate) use ui::SaveMenu;

pub(crate) struct SavePlugin;

impl Plugin for SavePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SaveMenu>()
            .add_systems(OnEnter(GameState::Loading), ui::reset_menu)
            .add_systems(OnExit(GameState::Paused), ui::reset_menu)
            .add_systems(OnExit(GameState::GameOver), ui::reset_menu)
            .add_systems(
                Update,
                (ui::menu_input, ui::rebuild, ui::color_buttons).chain(),
            );
    }
}

#[derive(Resource)]
struct PendingRestore(Snapshot);

fn roster(world: &World) -> Option<&Roster> {
    let handle = world.get_resource::<RosterHandle>()?;
    world.get_resource::<Assets<Roster>>()?.get(&handle.0)
}

fn character_index(world: &World, name: &str) -> Result<usize, String> {
    roster(world)
        .and_then(|roster| {
            roster
                .characters
                .iter()
                .position(|entry| entry.name == name)
        })
        .ok_or_else(|| format!("character {name:?} is unavailable"))
}

impl From<&Health> for HealthSave {
    fn from(health: &Health) -> Self {
        Self {
            current: health.current,
            max: health.max,
        }
    }
}

impl From<HealthSave> for Health {
    fn from(health: HealthSave) -> Self {
        Self {
            current: health.current,
            max: health.max,
        }
    }
}

fn capture(world: &mut World) -> Result<Snapshot, String> {
    let (player, health) = world
        .query::<(&Player, &Health)>()
        .single(world)
        .map_err(|_| "no active player to save")?;
    let roster = roster(world).ok_or("character roster is unavailable")?;
    let character = roster
        .characters
        .get(player.selected)
        .ok_or("invalid character selection")?;
    let player = PlayerSave {
        character: character.name.clone(),
        position: player.position.to_array(),
        health: health.into(),
    };
    let mut enemies = std::collections::BTreeMap::new();
    for (enemy, health) in world.query::<(&Enemy, Option<&Health>)>().iter(world) {
        let health = health.ok_or("enemy health is unavailable")?;
        if enemies
            .insert(
                enemy.spawn_index,
                EnemySave {
                    position: enemy.position.to_array(),
                    health: health.into(),
                },
            )
            .is_some()
        {
            return Err("duplicate enemy spawn index".into());
        }
    }
    let mut pickups: Vec<_> = world
        .query::<&Pickable>()
        .iter(world)
        .map(|pickup| PickupSave {
            kind: pickup.kind,
            position: pickup.position.to_array(),
        })
        .collect();
    pickups.sort_by(|a, b| {
        a.position[0]
            .total_cmp(&b.position[0])
            .then(a.position[1].total_cmp(&b.position[1]))
            .then(a.kind.cmp(&b.kind))
    });
    Ok(Snapshot {
        seed: world.resource::<MapSeed>().0,
        player,
        inventory: world.resource::<Inventory>().0.clone(),
        enemies,
        pickups,
    })
}

fn begin_load(world: &mut World, snapshot: Snapshot) -> Result<(), String> {
    // The roster loads during startup/general loading. If available now, reject
    // unknown characters before discarding the live world.
    if roster(world).is_some() {
        character_index(world, &snapshot.player.character)?;
    }
    world.insert_resource(MapSeed(snapshot.seed));
    world.insert_resource(PendingRestore(snapshot));
    world
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Loading);
    Ok(())
}

/// Run after fresh terrain and all player/enemy spawn commands have applied,
/// before the first Playing update. A regular new game has no pending restore.
pub(crate) fn restore_pending(world: &mut World) {
    let Some(PendingRestore(snapshot)) = world.remove_resource::<PendingRestore>() else {
        return;
    };
    if let Err(error) = restore(world, snapshot) {
        warn!("Load could not be applied to generated terrain: {error}");
        world.resource_mut::<SaveMenu>().set_notice(format!(
            "Load failed: {error}. Started a fresh game instead."
        ));
    }
}

#[cfg(test)]
mod tests;

fn restore(world: &mut World, mut snapshot: Snapshot) -> Result<(), String> {
    if world.resource::<MapSeed>().0 != snapshot.seed {
        return Err("generated terrain has a different seed".into());
    }
    let selected = character_index(world, &snapshot.player.character)?;
    let player = world
        .query_filtered::<Entity, With<Player>>()
        .single(world)
        .map_err(|_| "expected exactly one freshly spawned player")?;
    let player_position = Vec2::from_array(snapshot.player.position);
    let enemies: Vec<_> = world
        .query::<(Entity, &Enemy)>()
        .iter(world)
        .map(|(entity, enemy)| (entity, enemy.spawn_index))
        .collect();
    let map = world.resource::<CollisionMap>();
    if !map.can_stand(player_position)
        || snapshot.enemies.iter().any(|(index, enemy)| {
            !map.can_stand(Vec2::from_array(enemy.position))
                || !enemies.iter().any(|(_, fresh)| fresh == index)
        })
    {
        return Err("saved player or enemies do not match generated terrain".into());
    }
    let mut pickups = Vec::new();
    for (entity, pickup) in world.query::<(Entity, &Pickable)>().iter(world) {
        if let Some(index) = snapshot.pickups.iter().position(|saved| {
            saved.kind == pickup.kind && saved.position == pickup.position.to_array()
        }) {
            snapshot.pickups.swap_remove(index);
        } else {
            pickups.push(entity);
        }
    }
    if !snapshot.pickups.is_empty() {
        return Err("saved pickups do not match generated terrain".into());
    }

    // No mutation until the whole snapshot has been checked against the fresh
    // world. Keep freshly spawned renderer components and transient AI state.
    let mut entity = world.entity_mut(player);
    let mut movement = entity.get_mut::<Player>().expect("fresh player");
    movement.select(selected);
    movement.position = player_position;
    entity.insert((
        Name::new(snapshot.player.character),
        Health::from(snapshot.player.health),
    ));
    for (entity, index) in enemies {
        if let Some(saved) = snapshot.enemies.remove(&index) {
            let mut entity = world.entity_mut(entity);
            entity.get_mut::<Enemy>().expect("fresh enemy").position =
                Vec2::from_array(saved.position);
            entity.insert(Health::from(saved.health));
        } else {
            world.despawn(entity);
        }
    }
    for entity in pickups {
        world.despawn(entity);
    }
    world.resource_mut::<Inventory>().0 = snapshot.inventory;
    Ok(())
}
