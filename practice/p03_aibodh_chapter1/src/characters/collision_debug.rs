use bevy::{
    math::{Isometry2d, Isometry3d},
    prelude::*,
};

use super::{CharactersPlugin, camera::ground_position, motion::Player};
use crate::map::{
    LAND_TOP,
    collision::{CollisionMap, FOOT_RADIUS, Tile},
    generate::TILE_SIZE,
};

#[derive(Resource, Default)]
struct CollisionDebug(bool);

fn toggle(keys: Res<ButtonInput<KeyCode>>, mut debug: ResMut<CollisionDebug>) {
    if keys.just_pressed(KeyCode::F3) {
        debug.0 = !debug.0;
    }
}

fn draw(
    debug: Res<CollisionDebug>,
    mode: Res<CharactersPlugin>,
    map: Res<CollisionMap>,
    player: Single<&Player>,
    mut gizmos: Gizmos,
) {
    if !debug.0 {
        return;
    }
    let flat = Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2);
    let ground = |p| ground_position(p).with_y(LAND_TOP + 0.035);
    let color = Color::srgb(0.25, 1.0, 1.0);
    match *mode {
        CharactersPlugin::Sprites2d => {
            gizmos.circle_2d(
                Isometry2d::from_translation(player.position),
                FOOT_RADIUS,
                color,
            );
        }
        CharactersPlugin::Billboard3d => {
            gizmos.circle(
                Isometry3d::new(ground(player.position), flat),
                FOOT_RADIUS / TILE_SIZE,
                color,
            );
        }
    }
    for (center, tile) in map.tiles() {
        let (size, color) = match tile {
            Tile::Land => continue,
            Tile::Shore => (Vec2::splat(TILE_SIZE), Color::srgb(0.2, 0.9, 0.45)),
            Tile::Water => (Vec2::splat(TILE_SIZE), Color::srgb(0.3, 0.6, 1.0)),
            _ => (
                tile.half_extent().unwrap() * 2.0,
                Color::srgb(1.0, 0.3, 0.2),
            ),
        };
        match *mode {
            CharactersPlugin::Sprites2d => {
                gizmos.rect_2d(Isometry2d::from_translation(center), size, color);
            }
            CharactersPlugin::Billboard3d => {
                gizmos.rect(
                    Isometry3d::new(ground(center), flat),
                    size / TILE_SIZE,
                    color,
                );
            }
        }
    }
}

pub(crate) struct CollisionDebugPlugin;

impl Plugin for CollisionDebugPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<CollisionDebug>().add_systems(
            Update,
            (toggle, draw)
                .chain()
                .run_if(in_state(crate::game_state::GameState::Playing))
                .after(super::motion::apply_velocity),
        );
    }
}
