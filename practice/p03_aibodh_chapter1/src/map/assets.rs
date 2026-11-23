//! Adapted from AIBodh Chapter 2 (MIT); see ../../LICENSE-MIT.
use crate::{inventory::ItemKind, map::tilemap::TILEMAP};
use bevy::prelude::*;
use bevy_procedural_tilemaps::prelude::GridDelta;

#[derive(Clone)]
pub struct SpawnableAsset {
    /// Name of the sprite inside our tilemap atlas
    sprite_name: &'static str,
    /// Offset in grid coordinates (for multi-tile objects)
    pub(super) grid_offset: GridDelta,
    pub(super) pickup: Option<ItemKind>,
}

impl SpawnableAsset {
    pub fn sprite_name(&self) -> &'static str {
        self.sprite_name
    }

    pub fn new(sprite_name: &'static str) -> Self {
        Self {
            sprite_name,
            grid_offset: GridDelta::new(0, 0, 0),
            pickup: None,
        }
    }

    pub fn with_pickup(mut self, kind: ItemKind) -> Self {
        self.pickup = Some(kind);
        self
    }

    pub fn with_grid_offset(mut self, offset: GridDelta) -> Self {
        self.grid_offset = offset;
        self
    }
}

#[derive(Clone)]
pub struct TilemapHandles {
    pub image: Handle<Image>,
    pub layout: Handle<TextureAtlasLayout>,
}

impl TilemapHandles {
    pub fn sprite(&self, atlas_index: usize) -> Sprite {
        Sprite::from_atlas_image(
            self.image.clone(),
            TextureAtlas::from(self.layout.clone()).with_index(atlas_index),
        )
    }
}

pub fn prepare_tilemap_handles(
    asset_server: &Res<AssetServer>,
    atlas_layouts: &mut ResMut<Assets<TextureAtlasLayout>>,
    path: &'static str,
) -> TilemapHandles {
    let image = asset_server.load::<Image>(path);
    let mut layout = TextureAtlasLayout::new_empty(TILEMAP.atlas_size());
    for index in 0..TILEMAP.sprites.len() {
        layout.add_texture(TILEMAP.sprite_rect(index));
    }
    let layout = atlas_layouts.add(layout);

    TilemapHandles { image, layout }
}
