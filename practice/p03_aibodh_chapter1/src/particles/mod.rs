//! CPU simulation shared by both renderers; one unit quad is reused by every particle.
mod material;

use bevy::{prelude::*, sprite_render::Material2dPlugin};
use rand::{Rng, SeedableRng, rngs::StdRng};
use std::f32::consts::TAU;

use crate::{
    characters::{CharactersPlugin, FollowCamera},
    game_state::GameState,
    map::generate::TILE_SIZE,
};
use material::Glow;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ParticleSet;

pub(crate) struct ParticlePlugin;

impl Plugin for ParticlePlugin {
    fn build(&self, app: &mut App) {
        if matches!(
            app.world().resource::<CharactersPlugin>(),
            CharactersPlugin::Sprites2d
        ) {
            app.add_plugins(Material2dPlugin::<Glow>::default());
        } else {
            app.add_plugins(MaterialPlugin::<Glow>::default());
        }
        app.add_systems(Startup, prepare_quad).add_systems(
            Update,
            (advance, emit)
                .chain()
                .in_set(ParticleSet)
                .run_if(in_state(GameState::Playing)),
        );
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Layer {
    pub count: u32,
    pub lifetime: f32,
    pub speed: f32,
    pub spread: f32,
    pub radius: f32,
    pub size: f32,
    pub spin: f32,
    pub rise: f32,
    pub colors: [LinearRgba; 3],
}

#[derive(Component)]
pub(crate) struct Emitter {
    pub origin: Vec3,
    pub direction: Vec2,
    pub layer: Layer,
    // None is an immediate one-shot; a repeating timer supports a continuous emitter.
    pub interval: Option<Timer>,
}

#[derive(Component)]
pub(crate) struct Particle {
    velocity: Vec3,
    acceleration: Vec3,
    age: f32,
    lifetime: f32,
    angle: f32,
    spin: f32,
    size: f32,
    colors: [LinearRgba; 3],
    glow: Handle<Glow>,
}

impl Particle {
    fn visual(&self) -> (f32, LinearRgba) {
        let t = (self.age / self.lifetime).clamp(0.0, 1.0);
        let color = if t < 0.5 {
            self.colors[0].mix(&self.colors[1], t * 2.0)
        } else {
            self.colors[1].mix(&self.colors[2], t * 2.0 - 1.0)
        };
        (self.size * (1.0 - 0.8 * t), color)
    }
}

#[derive(Resource)]
struct Quad(Handle<Mesh>);

fn prepare_quad(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>) {
    commands.insert_resource(Quad(meshes.add(Rectangle::new(1.0, 1.0))));
}

fn emit(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<CharactersPlugin>,
    quad: Res<Quad>,
    mut materials: ResMut<Assets<Glow>>,
    mut emitters: Query<(Entity, &mut Emitter)>,
    camera: Query<&Transform, With<FollowCamera>>,
    mut rng: Local<ParticleRng>,
) {
    let (units, facing) = view_basis(*mode, &camera);
    for (entity, mut emitter) in &mut emitters {
        let bursts = match emitter.interval.as_mut() {
            // Limit catch-up after a stalled frame to avoid excessive allocations.
            Some(timer) => timer.tick(time.delta()).times_finished_this_tick().min(4),
            None => {
                commands.entity(entity).despawn();
                1
            }
        };
        let layer = emitter.layer;
        let rng = &mut rng.0;
        for _ in 0..bursts * layer.count {
            let direction =
                Vec2::from_angle(rng.gen_range(-1.0..1.0) * layer.spread).rotate(emitter.direction);
            let offset = Vec2::from_angle(rng.gen_range(0.0..TAU))
                * layer.radius
                * rng.r#gen::<f32>().sqrt();
            let spatial = |v: Vec2| match *mode {
                CharactersPlugin::Sprites2d => v.extend(0.0),
                CharactersPlugin::Billboard3d => Vec3::new(v.x, 0.0, -v.y),
            };
            let glow = materials.add(Glow {
                color: layer.colors[0],
            });
            let particle = Particle {
                velocity: spatial(direction) * layer.speed * units * rng.gen_range(0.78..1.22),
                acceleration: Vec3::Y * layer.rise * units,
                age: 0.0,
                lifetime: layer.lifetime * rng.gen_range(0.8..1.2),
                angle: rng.gen_range(0.0..TAU),
                spin: layer.spin * rng.gen_range(-1.0..1.0),
                size: layer.size * units * rng.gen_range(0.75..1.25),
                colors: layer.colors,
                glow: glow.clone(),
            };
            let transform = Transform::from_translation(emitter.origin + spatial(offset) * units)
                .with_rotation(facing * Quat::from_rotation_z(particle.angle))
                .with_scale(Vec3::splat(particle.size));
            let mut entity =
                commands.spawn((particle, transform, crate::game_state::SessionEntity));
            match *mode {
                CharactersPlugin::Sprites2d => {
                    entity.insert((Mesh2d(quad.0.clone()), MeshMaterial2d(glow)));
                }
                CharactersPlugin::Billboard3d => {
                    entity.insert((Mesh3d(quad.0.clone()), MeshMaterial3d(glow)));
                }
            }
        }
    }
}

fn view_basis(
    mode: CharactersPlugin,
    camera: &Query<&Transform, With<FollowCamera>>,
) -> (f32, Quat) {
    match mode {
        CharactersPlugin::Sprites2d => (1.0, Quat::IDENTITY),
        CharactersPlugin::Billboard3d => (
            TILE_SIZE.recip(),
            camera.single().map_or(Quat::IDENTITY, |t| t.rotation),
        ),
    }
}

fn advance(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<CharactersPlugin>,
    camera: Query<&Transform, With<FollowCamera>>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform), Without<FollowCamera>>,
    mut materials: ResMut<Assets<Glow>>,
) {
    let (_, facing) = view_basis(*mode, &camera);
    let dt = time.delta_secs();
    for (entity, mut particle, mut transform) in &mut particles {
        particle.age += dt;
        if particle.age >= particle.lifetime {
            commands.entity(entity).despawn();
            continue;
        }
        let acceleration = particle.acceleration;
        transform.translation += particle.velocity * dt + acceleration * (0.5 * dt * dt);
        particle.velocity += acceleration * dt;
        particle.angle += particle.spin * dt;
        transform.rotation = facing * Quat::from_rotation_z(particle.angle);
        let (size, color) = particle.visual();
        transform.scale = Vec3::splat(size);
        if let Some(mut glow) = materials.get_mut(&particle.glow) {
            glow.color = color;
        }
    }
}

struct ParticleRng(StdRng);

impl Default for ParticleRng {
    fn default() -> Self {
        Self(StdRng::from_entropy())
    }
}
