use bevy::{light::NotShadowCaster, math::Affine2, prelude::*};

use super::{
    CharactersPlugin, Roster, RosterHandle,
    camera::{FollowCamera, map_ground},
    config::Character,
    motion::Player,
};
use crate::map::generate::{TILE_SIZE, depth_from_feet};

#[derive(Component)]
pub(super) struct DisplayedCharacter(usize);

// Extract the same grid cell that Sprite's atlas uses, in normalized texture coordinates.
fn frame_uv(entry: &Character, frame: UVec2) -> Affine2 {
    let dimensions = Vec2::new(entry.atlas_columns as f32, entry.atlas_rows as f32);
    Affine2::from_scale_angle_translation(dimensions.recip(), 0.0, frame.as_vec2() / dimensions)
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw_player(
    mut commands: Commands,
    mode: Res<CharactersPlugin>,
    map: Res<crate::map::collision::CollisionMap>,
    handle: Res<RosterHandle>,
    rosters: Res<Assets<Roster>>,
    server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&Transform, (With<FollowCamera>, Without<Player>)>,
    mut player: Query<(
        Entity,
        &Player,
        &mut Transform,
        Option<&DisplayedCharacter>,
        Option<&mut Sprite>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
) {
    let (Some(roster), Ok((entity, player, mut transform, displayed, sprite, material))) =
        (rosters.get(&handle.0), player.single_mut())
    else {
        return;
    };
    let entry = &roster.characters[player.selected];
    let frame = player.frame(entry);
    let changed = displayed.is_none_or(|displayed| displayed.0 != player.selected);
    let height = player.lift * 20.0;
    match *mode {
        CharactersPlugin::Sprites2d => {
            transform.translation = Vec3::new(
                player.position.x,
                player.position.y + height + entry.tile_size as f32 / 3.0,
                depth_from_feet(player.position.y) + 0.0001,
            );
            let index = (frame.y * entry.atlas_columns + frame.x) as usize;
            if changed {
                let layout = layouts.add(TextureAtlasLayout::from_grid(
                    UVec2::splat(entry.tile_size),
                    entry.atlas_columns,
                    entry.atlas_rows,
                    None,
                    None,
                ));
                commands.entity(entity).insert(Sprite::from_atlas_image(
                    server.load(&entry.texture_path),
                    TextureAtlas { layout, index },
                ));
            } else if let Some(mut sprite) = sprite
                && let Some(atlas) = &mut sprite.texture_atlas
            {
                atlas.index = index;
            }
        }
        CharactersPlugin::Billboard3d => {
            let Ok(camera) = camera.single() else {
                return;
            };
            let size = entry.tile_size as f32 / TILE_SIZE;
            transform.rotation = camera.rotation;
            transform.scale = Vec3::splat(size);
            // Place the billboard from its feet, independent of the camera pivot.
            transform.translation = map_ground(player.position, &map)
                + Vec3::Y * (0.02 + height / TILE_SIZE)
                + camera.rotation * Vec3::Y * (size / 3.0);
            let uv = frame_uv(entry, frame);
            if let Some(material) = material {
                if let Some(mut mat) = materials.get_mut(&material.0) {
                    if changed {
                        mat.base_color_texture = Some(server.load(&entry.texture_path));
                    }
                    if mat.uv_transform != uv {
                        mat.uv_transform = uv;
                    }
                }
            } else {
                let material = materials.add(StandardMaterial {
                    base_color_texture: Some(server.load(&entry.texture_path)),
                    uv_transform: uv,
                    alpha_mode: AlphaMode::Mask(0.5),
                    unlit: true,
                    ..default()
                });
                commands.entity(entity).insert((
                    Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
                    MeshMaterial3d(material),
                    NotShadowCaster,
                ));
            }
        }
    }
    if changed {
        commands
            .entity(entity)
            .insert(DisplayedCharacter(player.selected));
    }
}
