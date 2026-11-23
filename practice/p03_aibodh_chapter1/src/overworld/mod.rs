//! Voxel terrain backend for the same game loop as the WFC maps.
mod mesh;
mod terrain;
mod village;

use std::{collections::HashMap, sync::Arc};

use crate::{
    characters::Player,
    game_state::{GameState, LoadingError},
    inventory::{ItemKind, Pickable},
    map::{
        MapReady, MapSeed,
        collision::CollisionMap,
        generate::{TILE_SIZE, VoxelDetails, voxel_details},
    },
};
use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::now_or_never},
};
use terrain::{Block, Column, MIN_Y, Terrain, hash};
use village::Villages;

const CHUNK_SIZE: i32 = 16;
const VIEW_RADIUS: i32 = 7;
const MAX_JOBS: usize = 6;
const MAX_Y: i32 = 100;

pub(crate) struct OverworldPlugin(pub u64);

impl Plugin for OverworldPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(MapSeed(self.0))
            .insert_resource(VoxelMode)
            .init_resource::<Chunks>()
            .add_systems(Startup, setup)
            .add_systems(OnEnter(GameState::Loading), rebuild)
            .add_systems(Update, finish_build.run_if(in_state(GameState::Loading)))
            .add_systems(
                Update,
                tour.before(crate::characters::MovementSet)
                    .run_if(in_state(GameState::Playing)),
            )
            .add_systems(
                Update,
                (stream, status)
                    .chain()
                    .after(crate::characters::ViewSet)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

#[derive(Resource)]
pub(crate) struct VoxelMode;

type Build = (Arc<WorldGen>, CollisionMap, u64);
#[derive(Resource)]
struct PendingBuild(Task<Result<Build, String>>);

const SIZE: UVec2 = UVec2::splat(481);
const CENTER: i32 = 240;

// Ground coordinates in the chapter gameplay are 64 pixels per voxel, +Y north.
fn logical(x: i32, z: i32) -> Vec2 {
    Vec2::new(x as f32, -z as f32) * TILE_SIZE
}

fn rebuild(
    mut commands: Commands,
    seed: Res<MapSeed>,
    mut chunks: ResMut<Chunks>,
    props: Query<Entity, With<VoxelPickup>>,
) {
    for (_, state) in chunks.0.drain() {
        if let ChunkState::Visible(root) = state {
            commands.entity(root).despawn();
        }
    }
    for prop in &props {
        commands.entity(prop).despawn();
    }
    commands.remove_resource::<CollisionMap>();
    commands.remove_resource::<MapReady>();
    commands.remove_resource::<Generator>();
    let seed = seed.0;
    commands.insert_resource(PendingBuild(AsyncComputeTaskPool::get().spawn(
        async move {
            let terrain = Terrain::new(seed);
            let world = Arc::new(WorldGen {
                terrain,
                villages: Villages::new(seed, &terrain),
                details: Some(voxel_details(seed)?),
            });
            let map = CollisionMap::from_voxel_surface_with_size(SIZE, |cell| {
                world.walkable_y(cell.x as i32 - CENTER, CENTER - cell.y as i32)
            });
            Ok((world, map, seed))
        },
    )));
}

#[derive(Component)]
struct VoxelPickup;

fn finish_build(
    mut commands: Commands,
    task: Option<ResMut<PendingBuild>>,
    mut error: ResMut<LoadingError>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Some(mut task) = task else {
        return;
    };
    let Some(result) = now_or_never(&mut task.0) else {
        return;
    };
    commands.remove_resource::<PendingBuild>();
    let (world, map, seed) = match result {
        Ok(build) => build,
        Err(message) => {
            error.0 = Some(message);
            return;
        }
    };
    let stop = Tour::new(&world, &map);
    let cube = meshes.add(Cuboid::new(0.25, 0.3, 0.25));
    let colors = [
        Color::srgb(0.19, 0.73, 0.25),
        Color::srgb(0.91, 0.29, 0.57),
        Color::srgb(0.88, 0.35, 0.14),
        Color::srgb(0.20, 0.47, 0.46),
    ];
    let mats = colors.map(|c| materials.add(c));
    let kinds = [
        ItemKind::Herb,
        ItemKind::Flower,
        ItemKind::Mushroom,
        ItemKind::Fern,
    ];
    for &(x, z, kind) in &world.details.as_ref().expect("WFC detail field").plants {
        let p = logical(x, z);
        if !map.can_stand(p) || world.villages.floor(x, z).is_some() {
            continue;
        }
        let index = kinds
            .iter()
            .position(|&item| item == kind)
            .expect("known plant kind");
        commands.spawn((
            VoxelPickup,
            Pickable::new(kind, p),
            Mesh3d(cube.clone()),
            MeshMaterial3d(mats[index].clone()),
            Transform::from_xyz(x as f32 + 0.5, map.floor_y(p) + 0.15, z as f32 + 0.5),
        ));
    }
    commands.insert_resource(stop);
    commands.insert_resource(Generator(world));
    commands.insert_resource(map);
    commands.insert_resource(MapReady { seed });
    info!("Voxel terrain ready (seed {seed})");
}

pub(crate) struct WorldGen {
    terrain: Terrain,
    villages: Villages,
    details: Option<VoxelDetails>,
}

struct Flora {
    trunk: Option<i32>,
    leaves: [Option<i32>; 25],
}

impl WorldGen {
    fn flora(&self, x: i32, z: i32) -> Flora {
        let mut flora = Flora {
            trunk: None,
            leaves: [None; 25],
        };
        for dz in -2..=2 {
            for dx in -2..=2 {
                let (tx, tz) = (x + dx, z + dz);
                let column = self.terrain.column(tx, tz);
                if !self.terrain.tree(tx, tz, column) {
                    continue;
                }
                let top = column.height + 4 + (hash(5, tx, 0, tz) & 1) as i32;
                if dx == 0 && dz == 0 {
                    flora.trunk = Some(top);
                }
                flora.leaves[((dz + 2) * 5 + dx + 2) as usize] = Some(top);
            }
        }
        flora
    }

    fn walkable_y(&self, x: i32, z: i32) -> Option<i32> {
        let c = self.terrain.column(x, z);
        let y = if let Some(y) = self.villages.floor(x, z) {
            y
        } else {
            if c.water.is_some()
                || self.terrain.tree(x, z, c)
                || self.terrain.body(x, c.height, z, c) == Block::Air
            {
                return None;
            }
            c.height
        };
        ((1..=2).all(|dy| {
            self.villages
                .block(x, y + dy, z)
                .is_none_or(|b| b == Block::Air)
        }))
        .then_some(y + 1)
    }

    pub(crate) fn camera_arm(&self, pivot: Vec3, back: Vec3, max: f32) -> f32 {
        let steps = (max / 0.12).ceil() as u32;
        for i in 1..=steps {
            let d = max * i as f32 / steps as f32;
            let p = pivot + back * d;
            for offset in [
                Vec3::ZERO,
                Vec3::X * 0.14,
                Vec3::NEG_X * 0.14,
                Vec3::Y * 0.14,
                Vec3::NEG_Y * 0.14,
                Vec3::Z * 0.14,
                Vec3::NEG_Z * 0.14,
            ] {
                let b = (p + offset).floor().as_ivec3();
                if !matches!(self.block(b.x, b.y, b.z), Block::Air | Block::Water) {
                    return (d - 0.14).max(0.25);
                }
            }
        }
        max
    }

    fn material_at(&self, x: i32, y: i32, z: i32, c: Column, flora: Option<&Flora>) -> Block {
        if let Some(b) = self.villages.block(x, y, z) {
            return b;
        }
        let base = self.terrain.body(x, y, z, c);
        if base == Block::Grass
            && c.biome == terrain::Biome::Plains
            && self
                .details
                .as_ref()
                .is_some_and(|details| details.dry_at(x, z))
        {
            return Block::DryGrass;
        }
        if base != Block::Air || y <= c.height {
            return base;
        }
        if let Some(f) = flora {
            if f.trunk.is_some_and(|top| y <= top) {
                return Block::Log;
            }
            if f.leaves.iter().enumerate().any(|(i, &top)| {
                top.is_some_and(|top| {
                    let radius = if y == top + 1 { 1 } else { 2 };
                    let dx = (i as i32 % 5 - 2).abs();
                    let dz = (i as i32 / 5 - 2).abs();
                    y >= top - 2 && y <= top + 1 && dx.max(dz) <= radius && dx + dz <= radius + 1
                })
            }) {
                return Block::Leaves;
            }
        }
        base
    }

    fn fill_column(&self, x: i32, z: i32, out: &mut [Block]) {
        let c = self.terrain.column(x, z);
        let f = self.flora(x, z);
        for (i, block) in out.iter_mut().enumerate() {
            *block = self.material_at(x, MIN_Y + i as i32, z, c, Some(&f));
        }
    }

    fn block(&self, x: i32, y: i32, z: i32) -> Block {
        let c = self.terrain.column(x, z);
        let f = (y > c.height && y <= MAX_Y).then(|| self.flora(x, z));
        self.material_at(x, y, z, c, f.as_ref())
    }
}

#[derive(Resource)]
pub(crate) struct Generator(pub(crate) Arc<WorldGen>);

#[derive(Resource, Default)]
struct Chunks(HashMap<IVec2, ChunkState>);

enum ChunkState {
    Working(Task<mesh::ChunkMesh>),
    Visible(Entity),
}

#[derive(Resource)]
struct Palette {
    opaque: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
}

#[derive(Component)]
struct Readout;

#[derive(Resource)]
struct Tour {
    targets: [Option<(i32, i32)>; 5],
    current: usize,
}

impl Tour {
    fn new(world: &WorldGen, map: &CollisionMap) -> Self {
        let spawn = world.villages.spawn();
        let mut targets = [None; 5];
        targets[0] = Some(spawn);
        let mut best = [i32::MAX; 5];
        for z in (-224i32..=224).step_by(4) {
            for x in (-224i32..=224).step_by(4) {
                let c = world.terrain.column(x, z);
                let index = if c.water.is_some() {
                    // Prefer the dry side of the shore over a teleport into water.
                    continue;
                } else if c.biome == terrain::Biome::SnowyPeaks {
                    1
                } else if c.biome == terrain::Biome::Forest {
                    2
                } else if c.biome == terrain::Biome::Desert {
                    3
                } else {
                    4
                };
                if index == 4
                    && ![(4, 0), (-4, 0), (0, 4), (0, -4)]
                        .into_iter()
                        .any(|(dx, dz)| world.terrain.column(x + dx, z + dz).water.is_some())
                {
                    continue;
                }
                let distance = x * x + z * z;
                if distance < best[index]
                    && (index == 4 || x.abs().max(z.abs()) > 32)
                    && map.can_stand(logical(x, z))
                {
                    best[index] = distance;
                    targets[index] = Some((x, z));
                }
            }
        }
        Self {
            targets,
            current: 0,
        }
    }
}

impl Tour {
    fn next(&self) -> usize {
        (1..=self.targets.len())
            .map(|offset| (self.current + offset) % self.targets.len())
            .find(|&i| self.targets[i].is_some())
            .unwrap_or(0)
    }
}

const STOPS: [&str; 5] = ["Village", "Snowy peaks", "Forest", "Desert", "Lakeshore"];

fn tour(keys: Res<ButtonInput<KeyCode>>, mut route: ResMut<Tour>, mut player: Single<&mut Player>) {
    if !keys.just_pressed(KeyCode::KeyT) {
        return;
    }
    route.current = route.next();
    let (x, z) = route.targets[route.current].expect("tour always includes spawn");
    player.position = logical(x, z);
}

fn setup(mut commands: Commands, mut materials: ResMut<Assets<StandardMaterial>>) {
    commands.insert_resource(Palette {
        opaque: materials.add(StandardMaterial {
            perceptual_roughness: 0.94,
            ..default()
        }),
        water: materials.add(StandardMaterial {
            alpha_mode: AlphaMode::Blend,
            perceptual_roughness: 0.25,
            ..default()
        }),
    });
    commands.insert_resource(GlobalAmbientLight {
        brightness: 220.,
        ..default()
    });
    commands.spawn((
        DirectionalLight {
            illuminance: 10000.,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::YXZ, -0.75, -0.9, 0.)),
    ));
    commands.spawn((
        Readout,
        Text::new("Generating chunks…"),
        TextFont {
            font_size: FontSize::Px(18.),
            ..default()
        },
        Node {
            position_type: PositionType::Absolute,
            left: Val::Px(12.),
            top: Val::Px(68.),
            padding: UiRect::all(Val::Px(7.)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.04, 0.09, 0.14, 0.70)),
    ));
}

fn chunk_at(position: Vec3) -> IVec2 {
    let xyz = position.floor().as_ivec3();
    IVec2::new(xyz.x.div_euclid(CHUNK_SIZE), xyz.z.div_euclid(CHUNK_SIZE))
}

fn stream(
    mut commands: Commands,
    world: Res<Generator>,
    palette: Res<Palette>,
    mut chunks: ResMut<Chunks>,
    player: Single<&Player>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    let center =
        chunk_at(crate::characters::ground_position(player.position) + Vec3::new(0.5, 0., 0.5));
    chunks.0.retain(|coord, state| {
        if (*coord - center).abs().max_element() <= VIEW_RADIUS + 1 {
            return true;
        }
        if let ChunkState::Visible(id) = state {
            commands.entity(*id).despawn();
        }
        false
    });
    for (coord, state) in &mut chunks.0 {
        let ChunkState::Working(task) = state else {
            continue;
        };
        let Some(meshes_cpu) = now_or_never(task) else {
            continue;
        };
        let root = commands
            .spawn((
                Transform::from_xyz(
                    (coord.x * CHUNK_SIZE) as f32,
                    0.,
                    (coord.y * CHUNK_SIZE) as f32,
                ),
                Visibility::default(),
            ))
            .id();
        for (mesh, material) in [
            (meshes_cpu.opaque, &palette.opaque),
            (meshes_cpu.water, &palette.water),
        ] {
            if let Some(mesh) = mesh {
                commands.spawn((
                    Mesh3d(meshes.add(mesh)),
                    MeshMaterial3d(material.clone()),
                    Transform::default(),
                    ChildOf(root),
                ));
            }
        }
        *state = ChunkState::Visible(root);
    }
    let active = chunks
        .0
        .values()
        .filter(|s| matches!(s, ChunkState::Working(_)))
        .count();
    if active >= MAX_JOBS {
        return;
    }
    let mut missing = Vec::new();
    for z in -VIEW_RADIUS..=VIEW_RADIUS {
        for x in -VIEW_RADIUS..=VIEW_RADIUS {
            let coord = center + IVec2::new(x, z);
            if !chunks.0.contains_key(&coord) {
                missing.push(coord);
            }
        }
    }
    missing.sort_unstable_by_key(|c| {
        let d = *c - center;
        d.x * d.x + d.y * d.y
    });
    let pool = AsyncComputeTaskPool::get();
    for coord in missing.into_iter().take(MAX_JOBS - active) {
        let world = Arc::clone(&world.0);
        chunks.0.insert(
            coord,
            ChunkState::Working(pool.spawn(async move { mesh::build(&world, coord) })),
        );
    }
}

fn status(
    map: Res<CollisionMap>,
    player: Single<&Player>,
    world: Res<Generator>,
    chunks: Res<Chunks>,
    route: Res<Tour>,
    mut text: Single<&mut Text, With<Readout>>,
) {
    let p = crate::characters::map_ground(player.position, &map)
        .floor()
        .as_ivec3();
    let biome = world.0.terrain.column(p.x, p.z).biome;
    let ready = chunks
        .0
        .values()
        .filter(|c| matches!(c, ChunkState::Visible(_)))
        .count();
    let content = format!(
        "Overworld  |  {biome:?}  |  {}, {}, {}  |  {} chunks  |  Next stop: {}\nWASD · Shift sprint · Ctrl cast · T tour · Esc pause",
        p.x,
        p.y,
        p.z,
        ready,
        STOPS[route.next()]
    );
    if text.0 != content {
        text.0 = content;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_coordinates_use_the_neighboring_chunk() {
        assert_eq!(chunk_at(Vec3::new(-0.1, 44., -16.1)), IVec2::new(-1, -2));
    }

    #[test]
    fn columns_match_runtime_collision_sampler() {
        let terrain = Terrain::new(42);
        let world = WorldGen {
            terrain,
            villages: Villages::new(42, &terrain),
            details: None,
        };
        for (x, z) in [(0, 0), (-16, -3), (-17, 42), (47, -96), (-111, 111)] {
            let mut values = [Block::Air; (MAX_Y - MIN_Y + 1) as usize];
            world.fill_column(x, z, &mut values);
            for (i, b) in values.into_iter().enumerate() {
                assert_eq!(b, world.block(x, MIN_Y + i as i32, z));
            }
        }
        assert_eq!(world.walkable_y(0, 0), Some(34));
    }
}
