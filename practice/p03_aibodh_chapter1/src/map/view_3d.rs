use bevy::{camera::ScalingMode, prelude::*};

use super::{
    block_art::{BlockArt, block_mesh},
    generate::{GRID_X, GRID_Y, MAP_SEED, create_generator},
};

// 一格对应一个 3D 世界单位，不沿用 2D 版的 32 像素尺寸。
pub(crate) const LAND_TOP: f32 = 1.0;
const LAKE_BED: f32 = 0.55;
const WATER_TOP: f32 = 0.8;
const CELL_COUNT: usize = (GRID_X * GRID_Y) as usize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
enum TileKind {
    Dirt,
    Grass,
    YellowGrass,
    Water,
    Tree,
    Rock,
    Plant,
    Stump,
}

impl TileKind {
    fn from_sprite(name: &str) -> Self {
        match name {
            "dirt" => Self::Dirt,
            name if name.starts_with("green_grass") => Self::Grass,
            name if name.starts_with("yellow_grass") => Self::YellowGrass,
            name if name.starts_with("water") => Self::Water,
            name if name.starts_with("small_tree") || name.starts_with("big_tree") => Self::Tree,
            name if name.starts_with("rock_") => Self::Rock,
            name if name.starts_with("plant_") => Self::Plant,
            name if name.starts_with("tree_stump_") => Self::Stump,
            _ => panic!("No 3D tile style for {name}"),
        }
    }
}

fn cell_center(x: u32, y: u32) -> Vec2 {
    Vec2::new(
        x as f32 + 0.5 - GRID_X as f32 / 2.0,
        GRID_Y as f32 / 2.0 - y as f32 - 0.5,
    )
}

struct BlockPalette {
    mesh: Handle<Mesh>,
    materials: [Handle<StandardMaterial>; BlockArt::ALL.len()],
    parent: Entity,
}

impl BlockPalette {
    fn spawn(&self, commands: &mut Commands, name: &str, art: BlockArt, center: Vec3, size: Vec3) {
        commands.spawn((
            Name::new(name.to_owned()),
            Mesh3d(self.mesh.clone()),
            MeshMaterial3d(self.materials[art as usize].clone()),
            Transform::from_translation(center).with_scale(size),
            ChildOf(self.parent),
        ));
    }

    fn spawn_prop(&self, commands: &mut Commands, kind: TileKind, name: &str, center: Vec2) {
        let origin = Vec3::new(center.x, LAND_TOP, center.y);
        let mut part = |name, art, x, bottom, z, width, height, depth| {
            self.spawn(
                commands,
                name,
                art,
                origin + Vec3::new(x, bottom + height * 0.5, z),
                Vec3::new(width, height, depth),
            );
        };
        match kind {
            TileKind::Tree => {
                let (trunk, crown) = if name.starts_with("big_tree") {
                    (1.25, 0.98)
                } else {
                    (0.9, 0.8)
                };
                part(name, BlockArt::Bark, 0.0, 0.0, 0.0, 0.31, trunk, 0.31);
                part(
                    "Tree canopy",
                    BlockArt::Leaves,
                    0.0,
                    trunk,
                    0.0,
                    crown,
                    0.52,
                    crown,
                );
                part(
                    "Tree canopy tip",
                    BlockArt::Leaves,
                    0.0,
                    trunk + 0.52,
                    0.0,
                    crown * 0.62,
                    0.37,
                    crown * 0.62,
                );
            }
            TileKind::Rock => {
                part(name, BlockArt::Stone, 0.0, 0.0, 0.0, 0.63, 0.19, 0.55);
                part(
                    "Rock upper block",
                    BlockArt::Stone,
                    -0.06,
                    0.19,
                    0.01,
                    0.38,
                    0.16,
                    0.36,
                );
            }
            TileKind::Plant => {
                part(name, BlockArt::Stem, 0.0, 0.0, 0.0, 0.07, 0.40, 0.07);
                part(
                    "Plant leaf",
                    BlockArt::Leaves,
                    -0.09,
                    0.15,
                    0.0,
                    0.11,
                    0.08,
                    0.12,
                );
                part(
                    "Plant leaf",
                    BlockArt::Leaves,
                    0.09,
                    0.23,
                    0.0,
                    0.11,
                    0.08,
                    0.12,
                );
                part(
                    "Flower head",
                    BlockArt::Flower,
                    0.0,
                    0.40,
                    0.0,
                    0.23,
                    0.16,
                    0.23,
                );
            }
            TileKind::Stump => part(name, BlockArt::Bark, 0.0, 0.0, 0.0, 0.46, 0.29, 0.46),
            _ => unreachable!("expected a decoration tile"),
        }
    }
}

pub(super) fn setup_map_3d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (mut generator, definitions) = create_generator(MAP_SEED);
    let (info, data) = generator
        .generate_grid()
        .expect("WFC exhausted its retry budget");
    info!(
        "Generated {} terrain cells for 3D view (seed {MAP_SEED}): {info:?}",
        data.iter().count()
    );

    let mut surfaces = [TileKind::Dirt; CELL_COUNT];
    let mut props = [None; CELL_COUNT];
    // WFC 从底层往上枚举；同一列只显示最高的地表类型，装饰仍单独生成。
    for (index, instance) in data.iter().enumerate() {
        let Some(asset) = definitions[instance.model_index].first() else {
            continue;
        };
        let name = asset.sprite_name();
        let kind = TileKind::from_sprite(name);
        let pos = generator.grid().pos_from_index(index);
        let cell = (pos.y * GRID_X + pos.x) as usize;
        match kind {
            TileKind::Dirt | TileKind::Grass | TileKind::YellowGrass | TileKind::Water => {
                surfaces[cell] = kind
            }
            _ => props[cell] = Some((kind, name)),
        }
    }

    let palette = BlockPalette {
        mesh: meshes.add(block_mesh()),
        materials: BlockArt::ALL.map(|art| materials.add(art.material(&mut images))),
        parent: commands
            .spawn((
                Name::new("Generated terrain (3D)"),
                Transform::default(),
                Visibility::default(),
            ))
            .id(),
    };
    for (i, (surface, prop)) in surfaces.into_iter().zip(props).enumerate() {
        let center = cell_center(i as u32 % GRID_X, i as u32 / GRID_X);
        let mut layer = |name, art, bottom, top| {
            palette.spawn(
                &mut commands,
                name,
                art,
                Vec3::new(center.x, (bottom + top) * 0.5, center.y),
                Vec3::new(1.0, top - bottom, 1.0),
            );
        };
        match surface {
            TileKind::Dirt => layer("dirt", BlockArt::Dirt, 0.0, LAND_TOP),
            TileKind::Grass => layer("grass", BlockArt::Grass, 0.0, LAND_TOP),
            TileKind::YellowGrass => layer("dry grass", BlockArt::DryGrass, 0.0, LAND_TOP),
            TileKind::Water => {
                layer("lake bed", BlockArt::Dirt, 0.0, LAKE_BED);
                layer("water", BlockArt::Water, LAKE_BED, WATER_TOP);
            }
            _ => unreachable!("expected a terrain tile"),
        }
        if let Some((kind, name)) = prop {
            palette.spawn_prop(&mut commands, kind, name, center);
        }
    }

    commands.insert_resource(ClearColor(Color::srgb(0.60, 0.76, 0.88)));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 170.0,
        ..default()
    });
    commands.spawn((
        Camera3d::default(),
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::AutoMin {
                min_width: 36.0,
                min_height: 28.0,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_xyz(18.0, 24.0, 26.0).looking_at(Vec3::new(0.0, 0.8, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-9.0, 14.0, 7.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_visible_model_has_a_category() {
        let (_, definitions) = create_generator(MAP_SEED);
        let mut seen = [false; 8];
        for asset in definitions.iter().filter_map(|assets| assets.first()) {
            seen[TileKind::from_sprite(asset.sprite_name()) as usize] = true;
        }
        assert!(seen.into_iter().all(|present| present));
    }

    #[test]
    fn startup_spawns_centered_voxels_with_textures_and_recessed_water() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, setup_map_3d);
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<(
            &Name,
            &Mesh3d,
            &MeshMaterial3d<StandardMaterial>,
            &Transform,
        )>();
        let blocks: Vec<_> = query.iter(world).collect();
        let materials = world.resource::<Assets<StandardMaterial>>();
        assert!(blocks.iter().all(|(_, mesh, mat, _)| {
            mesh.0 == blocks[0].1.0 && materials.get(&mat.0).unwrap().base_color_texture.is_some()
        }));
        let ground: Vec<_> = blocks
            .iter()
            .filter(|(_, _, _, t)| t.translation.y - t.scale.y * 0.5 == 0.0)
            .collect();
        assert_eq!(ground.len(), CELL_COUNT);
        let sum: Vec3 = ground.iter().map(|(_, _, _, t)| t.translation).sum();
        assert_eq!((sum.x, sum.z), (0.0, 0.0));

        let top = |t: &Transform| t.translation.y + t.scale.y * 0.5;
        let water: Vec<_> = blocks
            .iter()
            .filter(|(name, _, _, _)| name.as_str() == "water")
            .collect();
        assert!(!water.is_empty());
        for (_, _, _, t) in water {
            assert!(top(t) < LAND_TOP);
            let bed = ground
                .iter()
                .find(|(_, _, _, bed)| bed.translation.xz() == t.translation.xz())
                .unwrap();
            assert!((top(bed.3) - (t.translation.y - t.scale.y * 0.5)).abs() < 1e-6);
        }
    }
}
