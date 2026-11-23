//! Chapter 8: swept gameplay hitboxes, independent of the particle renderer.
use bevy::prelude::*;

use crate::{
    characters::{Player, ViewSet},
    enemy::{Enemy, EnemySet},
    game_state::GameState,
    map::collision::{CollisionMap, FOOT_RADIUS},
    particles::ParticleSet,
};

#[derive(Component)]
pub(crate) struct Health {
    pub(crate) current: f32,
    pub(crate) max: f32,
}

impl Health {
    pub(crate) fn new(max: f32) -> Self {
        Self { current: max, max }
    }

    pub(crate) fn set_max(&mut self, max: f32) {
        if self.max != max {
            self.current = self.current / self.max * max;
            self.max = max;
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Side {
    Player,
    Enemy,
}

#[derive(Component)]
pub(crate) struct Projectile {
    pub position: Vec2,
    pub velocity: Vec2,
    pub remaining: f32,
    pub damage: f32,
    pub radius: f32,
    pub side: Side,
}

#[derive(EntityEvent)]
struct Hit {
    entity: Entity,
    damage: f32,
}

#[derive(Component)]
struct HealthLabel;

pub(crate) struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.add_observer(apply_hit)
            .add_systems(
                OnExit(GameState::Loading),
                spawn_hud.in_set(crate::game_state::SpawnSet),
            )
            .add_systems(
                Update,
                (fly, update_hud)
                    .chain()
                    .after(ViewSet)
                    .after(EnemySet)
                    .before(ParticleSet)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn fly(
    mut commands: Commands,
    map: Res<CollisionMap>,
    time: Res<Time>,
    players: Query<(Entity, &Player, &Health)>,
    enemies: Query<(Entity, &Enemy, &Health)>,
    mut shots: Query<(Entity, &mut Projectile)>,
) {
    for (id, mut shot) in &mut shots {
        let dt = time.delta_secs().min(shot.remaining);
        let delta = shot.velocity * dt;
        let steps = (delta.length() / 3.0).ceil().max(1.) as u32;
        let mut expired = false;
        for i in 0..=steps {
            let p = shot.position + delta * (i as f32 / steps as f32);
            if !map.can_stand(p) {
                expired = true;
                break;
            }
            let radius_sq = (shot.radius + FOOT_RADIUS).powi(2);
            let hit = match shot.side {
                Side::Player => enemies
                    .iter()
                    .find(|(_, e, hp)| {
                        hp.current > 0. && e.position.distance_squared(p) <= radius_sq
                    })
                    .map(|(id, _, _)| id),
                Side::Enemy => players
                    .iter()
                    .find(|(_, e, hp)| {
                        hp.current > 0.
                            && e.lift < 0.6
                            && e.position.distance_squared(p) <= radius_sq
                    })
                    .map(|(id, _, _)| id),
            };
            if let Some(entity) = hit {
                commands.trigger(Hit {
                    entity,
                    damage: shot.damage,
                });
                expired = true;
                break;
            }
        }
        shot.position += delta;
        shot.remaining -= dt;
        if expired || shot.remaining <= 0. {
            commands.entity(id).despawn();
        }
    }
}

fn apply_hit(
    hit: On<Hit>,
    mut commands: Commands,
    mut victims: Query<(&mut Health, Has<Player>)>,
    mut next: ResMut<NextState<GameState>>,
    mut cue: MessageWriter<crate::sound::SoundCue>,
) {
    let Ok((mut hp, is_player)) = victims.get_mut(hit.entity) else {
        return;
    };
    if hp.current <= 0. {
        return;
    }
    hp.current = (hp.current - hit.damage).max(0.);
    cue.write(crate::sound::SoundCue::Hit);
    if hp.current == 0. {
        if is_player {
            cue.write(crate::sound::SoundCue::PlayerDeath);
            next.set(GameState::GameOver);
        } else {
            cue.write(crate::sound::SoundCue::EnemyDeath);
            commands.entity(hit.entity).despawn();
        }
    }
}

fn spawn_hud(mut commands: Commands) {
    commands.spawn((
        HealthLabel,
        crate::game_state::SessionEntity,
        Text::new(""),
        TextFont {
            font_size: FontSize::Px(22.),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.),
            bottom: Val::Px(12.),
            padding: UiRect::all(Val::Px(10.)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.05, 0.03, 0.03, 0.83)),
    ));
}

fn update_hud(
    player: Single<&Health, With<Player>>,
    enemies: Query<&Health, With<Enemy>>,
    mut hud: Single<&mut Text, With<HealthLabel>>,
) {
    let alive = enemies.iter().filter(|hp| hp.current > 0.).count();
    let value = format!(
        "HP {:.0}/{:.0}  |  Reapers left: {alive}",
        player.current, player.max
    );
    if hud.0 != value {
        hud.0 = value;
    }
}
