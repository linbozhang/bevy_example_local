//! Chapter 7: ground-plane pursuit and spell casting, independent of the renderer.
mod view;

use std::collections::VecDeque;

use bevy::prelude::*;

use crate::{
    characters::{CharactersPlugin, MovementSet, Player, Roster, RosterHandle},
    combat::{Health, Side},
    game_state::GameState,
    magic::{Power, spawn_spell},
    map::collision::{CollisionMap, FOOT_RADIUS, Tile},
    particles::ParticleSet,
};

const DETECTION_RANGE: f32 = 320.0;
const ATTACK_RANGE: f32 = 116.0;
const ATTACK_HYSTERESIS: f32 = 16.0;
const REPATH_INTERVAL: f32 = 0.5;
const COOLDOWN: f32 = 2.0;
const REAPER: &str = "Graveyard Reaper";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
enum Behavior {
    #[default]
    Idle,
    Chase,
    Attack,
}

#[derive(Component, Default)]
pub(crate) struct Enemy {
    pub position: Vec2,
    pub(crate) spawn_index: usize,
    direction: Vec2,
    behavior: Behavior,
    waypoints: VecDeque<Vec2>,
    repath_in: f32,
    cooldown: f32,
    walk_clock: f32,
    speed: f32,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct EnemySet;

pub(crate) struct EnemyPlugin;

impl Plugin for EnemyPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            OnExit(GameState::Loading),
            spawn.in_set(crate::game_state::SpawnSet),
        )
        .add_systems(
            Update,
            (update, view::draw)
                .chain()
                .in_set(EnemySet)
                .after(MovementSet)
                .before(ParticleSet)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

fn spawn(
    mut commands: Commands,
    map: Res<CollisionMap>,
    mode: Res<CharactersPlugin>,
    roster: Res<Assets<Roster>>,
    handle: Res<RosterHandle>,
    server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let player = map.spawn_point();
    let entry = roster
        .get(&handle.0)
        .and_then(|r| r.characters.iter().find(|c| c.name == REAPER));
    if matches!(*mode, CharactersPlugin::Sprites2d) && entry.is_none() {
        warn!("Cannot spawn 2D enemies: the roster has no {REAPER}");
        return;
    }
    let speed = entry.map_or(80.0, |entry| entry.base_move_speed * 0.68);
    let mut chosen = Vec::new();
    let rendering = view::EnemyArt::new(
        *mode,
        entry,
        &server,
        &mut layouts,
        &mut meshes,
        &mut materials,
    );
    for (spawn_index, offset) in [Vec2::new(170.0, 72.0), Vec2::new(-190.0, 120.0)]
        .into_iter()
        .enumerate()
    {
        let desired = player + offset;
        let mut candidates: Vec<_> = map
            .tiles()
            .filter(|(p, tile)| {
                *tile == Tile::Land
                    && map.can_stand(*p)
                    && p.distance(player) >= ATTACK_RANGE + ATTACK_HYSTERESIS
                    && p.distance(player) <= DETECTION_RANGE
                    && chosen
                        .iter()
                        .all(|&other| p.distance(other) > FOOT_RADIUS * 2.0)
            })
            .map(|(p, _)| p)
            .collect();
        candidates.sort_by(|a, b| {
            a.distance_squared(desired)
                .total_cmp(&b.distance_squared(desired))
        });
        let Some(position) = candidates
            .into_iter()
            .find(|&p| !map.path(p, player).is_empty())
        else {
            continue;
        };
        chosen.push(position);
        let id = commands
            .spawn((
                Name::new(REAPER),
                crate::game_state::SessionEntity,
                Enemy {
                    position,
                    spawn_index,
                    speed,
                    direction: (player - position).normalize_or_zero(),
                    ..default()
                },
                Health::new(entry.map_or(125.0, |c| c.max_health)),
                Transform::default(),
                Visibility::default(),
            ))
            .id();
        rendering.attach(&mut commands, id);
        info!("Spawned {REAPER} at {position:?}");
    }
}

fn update(
    mut commands: Commands,
    mode: Res<CharactersPlugin>,
    map: Res<CollisionMap>,
    time: Res<Time>,
    player: Single<&Player>,
    mut enemies: Query<(Entity, &mut Enemy)>,
    mut cue: MessageWriter<crate::sound::SoundCue>,
) {
    let dt = time.delta_secs();
    let mut positions: Vec<_> = enemies.iter().map(|(id, e)| (id, e.position)).collect();
    for (id, mut enemy) in &mut enemies {
        let old = enemy.position;
        let to_player = player.position - old;
        let distance = to_player.length();
        enemy.cooldown = (enemy.cooldown - dt).max(0.0);
        let range = ATTACK_RANGE
            + if enemy.behavior == Behavior::Attack {
                ATTACK_HYSTERESIS
            } else {
                0.0
            };
        let next = if distance > DETECTION_RANGE {
            Behavior::Idle
        } else if distance <= range && map.clear_segment(old, player.position) {
            Behavior::Attack
        } else {
            Behavior::Chase
        };
        if next != enemy.behavior {
            enemy.waypoints.clear();
            enemy.repath_in = 0.0;
            enemy.behavior = next;
        }
        match next {
            Behavior::Idle => {}
            Behavior::Attack => {
                if let Some(direction) = to_player.try_normalize() {
                    enemy.direction = direction;
                    if enemy.cooldown == 0.0 && distance <= ATTACK_RANGE {
                        spawn_spell(
                            &mut commands,
                            *mode,
                            Power::Shadow,
                            old,
                            direction,
                            24.0,
                            Side::Enemy,
                        );
                        enemy.cooldown = COOLDOWN;
                        cue.write(crate::sound::SoundCue::EnemyCast);
                    }
                }
            }
            Behavior::Chase => {
                enemy.repath_in -= dt;
                if enemy.repath_in <= 0.0 {
                    // Keep heading for the current waypoint if it is still usable;
                    // replace the remaining route so moving targets do not go stale.
                    let route = enemy
                        .waypoints
                        .front()
                        .copied()
                        .filter(|&wp| map.clear_segment(old, wp))
                        .map(|wp| (wp, map.path(wp, player.position)))
                        .filter(|(_, route)| !route.is_empty());
                    enemy.waypoints = if let Some((wp, mut route)) = route {
                        if route.front() != Some(&wp) {
                            route.push_front(wp);
                        }
                        route
                    } else {
                        map.path(old, player.position)
                    };
                    enemy.repath_in = REPATH_INTERVAL;
                }
                while enemy
                    .waypoints
                    .front()
                    .is_some_and(|wp| wp.distance_squared(old) < 1.0)
                {
                    enemy.waypoints.pop_front();
                }
                if let Some(&wp) = enemy.waypoints.front() {
                    let offset = wp - old;
                    let step = offset.clamp_length_max(enemy.speed * dt);
                    let others = std::iter::once(player.position).chain(
                        positions
                            .iter()
                            .filter_map(|&(other, p)| (other != id).then_some(p)),
                    );
                    enemy.position = map.move_circle_avoiding(old, step, others);
                    if let Some(dir) = (enemy.position - old).try_normalize() {
                        enemy.direction = dir;
                    }
                }
            }
        }
        enemy.walk_clock = if enemy.position.distance_squared(old) > 1e-6 {
            enemy.walk_clock + dt
        } else {
            0.0
        };
        if let Some((_, pos)) = positions.iter_mut().find(|(other, _)| *other == id) {
            *pos = enemy.position;
        }
    }
}
