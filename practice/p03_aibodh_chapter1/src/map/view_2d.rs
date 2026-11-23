use bevy::prelude::*;

use crate::inventory::Pickable;

use super::{
    MapTile, TerrainRoot,
    assets::prepare_tilemap_handles,
    collision::CollisionMap,
    generate::{GRID_Z, TILE_SIZE, TILEMAP_IMAGE, TerrainBuild, depth_from_feet},
    tilemap::TILEMAP,
};

pub(super) fn spawn_map_2d(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    terrain: Res<TerrainBuild>,
) {
    let handles = prepare_tilemap_handles(&server, &mut layouts, TILEMAP_IMAGE);
    let sprites: Vec<_> = (0..TILEMAP.sprites.len())
        .map(|index| handles.sprite(index))
        .collect();
    let parent = commands
        .spawn((
            TerrainRoot,
            Name::new("Generated terrain (2D)"),
            Transform::default(),
            Visibility::default(),
        ))
        .id();
    for (pos, z, instance) in terrain.cells() {
        let assets = &terrain.definitions[instance.model_index];
        let Some(base) = assets.first() else { continue };
        let center = CollisionMap::center(pos);
        // Canopy sprites inherit the base cell depth, even across chunk seams.
        let depth = if z == GRID_Z - 1 {
            depth_from_feet(center.y)
        } else {
            z as f32 + 0.5
        };
        let prop_parent = if let Some(kind) = base.pickup {
            commands
                .spawn((
                    MapTile(pos),
                    Pickable::new(kind, center),
                    Transform::default(),
                    Visibility::default(),
                    ChildOf(parent),
                ))
                .id()
        } else {
            parent
        };
        for asset in assets {
            let offset = asset.grid_offset;
            let translation = (center + Vec2::new(offset.dx as f32, offset.dy as f32) * TILE_SIZE)
                .extend(depth + offset.dz as f32);
            let atlas_index = TILEMAP
                .sprite_index(asset.sprite_name())
                .expect("known terrain sprite");
            commands.spawn((
                sprites[atlas_index].clone(),
                Transform::from_translation(translation)
                    .with_scale(
                        (Vec2::splat(TILE_SIZE) / TILEMAP.tile_size().as_vec2()).extend(1.0),
                    )
                    .with_rotation(Quat::from_rotation_z(instance.rotation.rad())),
                ChildOf(prop_parent),
            ));
        }
    }
}
