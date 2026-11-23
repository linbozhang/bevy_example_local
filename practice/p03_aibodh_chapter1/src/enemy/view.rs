use bevy::prelude::*;

use super::Enemy;
use crate::{
    characters::{Action, Character, CharactersPlugin, ground_position},
    map::generate::depth_from_feet,
};

pub(super) enum EnemyArt {
    Sprite {
        image: Handle<Image>,
        layout: Handle<TextureAtlasLayout>,
        walk: WalkClip,
    },
    Model {
        cube: Handle<Mesh>,
        cloth: Handle<StandardMaterial>,
        trim: Handle<StandardMaterial>,
        eyes: Handle<StandardMaterial>,
    },
}

#[derive(Component, Clone, Copy)]
pub(super) struct WalkClip {
    first: u32,
    directional: bool,
    columns: u32,
    frames: u32,
    seconds: f32,
    size: f32,
}

#[derive(Component)]
pub(super) struct Leg {
    left: bool,
    home: Vec3,
}

impl EnemyArt {
    pub fn new(
        mode: CharactersPlugin,
        entry: Option<&Character>,
        server: &AssetServer,
        layouts: &mut Assets<TextureAtlasLayout>,
        meshes: &mut Assets<Mesh>,
        materials: &mut Assets<StandardMaterial>,
    ) -> Self {
        match mode {
            CharactersPlugin::Sprites2d => {
                let entry = entry.expect("2D enemy requires its roster entry");
                let clip = &entry.animations[&Action::Walk];
                Self::Sprite {
                    image: server.load(&entry.texture_path),
                    layout: layouts.add(TextureAtlasLayout::from_grid(
                        UVec2::splat(entry.tile_size),
                        entry.atlas_columns,
                        entry.atlas_rows,
                        None,
                        None,
                    )),
                    walk: WalkClip {
                        first: clip.start_row,
                        directional: clip.directional,
                        columns: entry.atlas_columns,
                        frames: clip.frame_count,
                        seconds: clip.frame_time,
                        size: entry.tile_size as f32,
                    },
                }
            }
            CharactersPlugin::Billboard3d => Self::Model {
                cube: meshes.add(Cuboid::new(1., 1., 1.)),
                cloth: materials.add(Color::srgb(0.28, 0.14, 0.40)),
                trim: materials.add(Color::srgb(0.10, 0.07, 0.16)),
                eyes: materials.add(StandardMaterial {
                    base_color: Color::srgb(0.77, 0.52, 1.0),
                    emissive: LinearRgba::rgb(0.92, 0.30, 2.0),
                    ..default()
                }),
            },
        }
    }

    pub fn attach(&self, commands: &mut Commands, id: Entity) {
        match self {
            Self::Sprite {
                image,
                layout,
                walk,
            } => {
                commands.entity(id).insert((
                    *walk,
                    Sprite::from_atlas_image(
                        image.clone(),
                        TextureAtlas {
                            layout: layout.clone(),
                            index: 0,
                        },
                    ),
                ));
            }
            Self::Model {
                cube,
                cloth,
                trim,
                eyes,
            } => {
                // Forward is model-local -Z. Feet, torso, hood, face and glowing eyes are solid meshes.
                for (name, material, at, size, leg) in [
                    ("Robe", cloth, [0., 0.52, 0.], [0.53, 0.66, 0.36], None),
                    ("Hood", cloth, [0., 1.02, 0.], [0.57, 0.52, 0.50], None),
                    ("Face", trim, [0., 1.00, -0.251], [0.39, 0.30, 0.03], None),
                    (
                        "Left eye",
                        eyes,
                        [-0.11, 1.04, -0.274],
                        [0.075, 0.06, 0.024],
                        None,
                    ),
                    (
                        "Right eye",
                        eyes,
                        [0.11, 1.04, -0.274],
                        [0.075, 0.06, 0.024],
                        None,
                    ),
                    (
                        "Left arm",
                        cloth,
                        [-0.32, 0.53, 0.],
                        [0.14, 0.49, 0.20],
                        None,
                    ),
                    (
                        "Right arm",
                        cloth,
                        [0.32, 0.53, 0.],
                        [0.14, 0.49, 0.20],
                        None,
                    ),
                    (
                        "Left boot",
                        trim,
                        [-0.14, 0.14, 0.],
                        [0.19, 0.28, 0.28],
                        Some(true),
                    ),
                    (
                        "Right boot",
                        trim,
                        [0.14, 0.14, 0.],
                        [0.19, 0.28, 0.28],
                        Some(false),
                    ),
                ] {
                    let home = Vec3::from_array(at);
                    let mut part = commands.spawn((
                        Name::new(name),
                        Mesh3d(cube.clone()),
                        MeshMaterial3d(material.clone()),
                        Transform::from_translation(home).with_scale(Vec3::from_array(size)),
                        ChildOf(id),
                    ));
                    if let Some(left) = leg {
                        part.insert(Leg { left, home });
                    }
                }
            }
        }
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn draw(
    mut enemies: Query<(&Enemy, &mut Transform, Option<(&WalkClip, &mut Sprite)>), Without<Leg>>,
    mut legs: Query<(&Leg, &ChildOf, &mut Transform), Without<Enemy>>,
) {
    for (enemy, mut t, sprite) in &mut enemies {
        let d = enemy.direction;
        if let Some((walk, mut sprite)) = sprite {
            let row = if !walk.directional {
                0
            } else if d.x.abs() > d.y.abs() {
                if d.x < 0. { 1 } else { 3 }
            } else if d.y > 0. {
                0
            } else {
                2
            };
            let col = (enemy.walk_clock / walk.seconds) as u32 % walk.frames;
            if let Some(atlas) = &mut sprite.texture_atlas {
                atlas.index = ((walk.first + row) * walk.columns + col) as usize;
            }
            t.translation = Vec3::new(
                enemy.position.x,
                enemy.position.y + walk.size / 3.0,
                depth_from_feet(enemy.position.y) + 0.0001,
            );
        } else {
            t.translation = ground_position(enemy.position);
            t.rotation = Quat::from_rotation_y((-d.x).atan2(d.y));
        }
    }
    for (leg, parent, mut t) in &mut legs {
        if let Ok((enemy, _, _)) = enemies.get(parent.parent()) {
            let phase = (enemy.walk_clock * 15.0).sin() * if leg.left { -1.0 } else { 1.0 };
            t.translation = leg.home + Vec3::new(0.0, phase.max(0.0) * 0.065, phase * 0.12);
        }
    }
}
