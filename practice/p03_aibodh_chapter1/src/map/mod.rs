//! Chapter 2：五层 WFC 地形；z 是图层，不是可行走高度。
mod assets;
pub mod generate;
mod models;
mod rules;
mod sockets;
mod tilemap;

use bevy::prelude::*;

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, generate::setup_map);
    }
}
