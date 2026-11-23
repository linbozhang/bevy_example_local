//! 同一套五层 WFC 地图规则，提供 2D 精灵和 3D 方块两种视图。
mod assets;
mod block_art;
pub(crate) mod collision;
pub mod generate;
mod models;
mod navigation;
mod rules;
mod sockets;
mod tilemap;
mod view_2d;
mod view_3d;

use bevy::{
    prelude::*,
    tasks::{AsyncComputeTaskPool, Task, futures::now_or_never},
};

use crate::game_state::{GameState, LoadingError};
use collision::CollisionMap;
use generate::{MAP_SEED, TerrainBuild, build_terrain};

pub(crate) use view_3d::{CameraObstacle, LAND_TOP};

/// Insert before entering Loading; the build snapshots this value on entry.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MapSeed(pub u64);

impl Default for MapSeed {
    fn default() -> Self {
        Self(MAP_SEED)
    }
}

/// Present only once the terrain, pickups and matching collision are installed.
/// Save this seed, not a potentially edited request for a future generation.
#[derive(Resource, Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MapReady {
    pub seed: u64,
}

/// Stable global tile ID, independent of render mode and entity spawn order.
/// Combined with the active seed, identifies a generated pickup for saves.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MapTile(pub UVec2);

#[derive(Component)]
struct TerrainRoot;

#[derive(Resource)]
struct MapTask(Task<Result<TerrainBuild, String>>);

pub enum MapPlugin {
    Sprites2d,
    Blocks3d,
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapSeed>()
            .add_systems(OnEnter(GameState::Loading), start_build);
        match self {
            Self::Sprites2d => app.add_systems(
                Update,
                (
                    poll_build,
                    (view_2d::spawn_map_2d, finish_build)
                        .chain()
                        .run_if(resource_exists::<TerrainBuild>),
                )
                    .chain()
                    .run_if(in_state(GameState::Loading)),
            ),
            Self::Blocks3d => app.add_systems(
                Update,
                (
                    poll_build,
                    (view_3d::spawn_map_3d, finish_build)
                        .chain()
                        .run_if(resource_exists::<TerrainBuild>),
                )
                    .chain()
                    .run_if(in_state(GameState::Loading)),
            ),
        };
    }
}

fn start_build(
    mut commands: Commands,
    roots: Query<Entity, With<TerrainRoot>>,
    seed: Res<MapSeed>,
) {
    for root in &roots {
        commands.entity(root).despawn();
    }
    commands.remove_resource::<MapReady>();
    commands.remove_resource::<CollisionMap>();
    let MapSeed(seed) = *seed;
    commands.insert_resource(MapTask(
        AsyncComputeTaskPool::get().spawn(async move { build_terrain(seed) }),
    ));
}

fn poll_build(
    mut commands: Commands,
    task: Option<ResMut<MapTask>>,
    mut error: ResMut<LoadingError>,
) {
    let Some(mut task) = task else { return };
    let Some(result) = now_or_never(&mut task.0) else {
        return;
    };
    commands.remove_resource::<MapTask>();
    match result {
        Ok(terrain) => commands.insert_resource(terrain),
        Err(message) => {
            error!("{message}");
            error.0.get_or_insert(message);
        }
    }
}

fn finish_build(mut commands: Commands) {
    // Move collision out of the transient build without cloning it. Deferred
    // order puts readiness after the renderer's entity commands.
    commands.queue(|world: &mut World| {
        let build = world.remove_resource::<TerrainBuild>().unwrap();
        world.insert_resource(build.collision);
        world.insert_resource(MapReady { seed: build.seed });
        info!("Stitched terrain ready (seed {})", build.seed);
    });
}

#[cfg(test)]
mod tests;
