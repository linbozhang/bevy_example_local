//! 同一套五层 WFC 地图规则，提供 2D 精灵和 3D 方块两种视图。
mod assets;
mod block_art;
pub mod generate;
mod models;
mod rules;
mod sockets;
mod tilemap;
mod view_3d;

use bevy::prelude::*;

pub(crate) use view_3d::{CameraObstacle, LAND_TOP};

pub enum MapPlugin {
    Sprites2d,
    Blocks3d,
}

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        match self {
            Self::Sprites2d => app.add_systems(Startup, generate::setup_map),
            Self::Blocks3d => app.add_systems(Startup, view_3d::setup_map_3d),
        };
    }
}
