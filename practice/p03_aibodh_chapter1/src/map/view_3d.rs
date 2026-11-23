use crate::inventory::{ItemKind, Pickable};
use bevy::prelude::*;

use super::{
    MapTile, TerrainRoot,
    block_art::{BlockArt, block_mesh},
    collision::{CollisionMap, Tile},
    generate::{COUNT, TILE_SIZE, TerrainBuild},
};

// 一格对应一个 3D 世界单位，不沿用 2D 版的 32 像素尺寸。
pub(crate) const LAND_TOP: f32 = 1.0;
const LAKE_BED: f32 = 0.55;
const WATER_TOP: f32 = 0.8;
// All generated blocks are axis-aligned under an identity terrain root.
#[derive(Component)]
pub(crate) struct CameraObstacle;

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

struct BlockPalette {
    mesh: Handle<Mesh>,
    materials: [Handle<StandardMaterial>; BlockArt::ALL.len()],
    parent: Entity,
}

impl BlockPalette {
    fn block(&self, name: &str, art: BlockArt, center: Vec3, size: Vec3) -> impl Bundle {
        (
            Name::new(name.to_owned()),
            Mesh3d(self.mesh.clone()),
            MeshMaterial3d(self.materials[art as usize].clone()),
            Transform::from_translation(center).with_scale(size),
        )
    }

    fn spawn(&self, commands: &mut Commands, name: &str, art: BlockArt, center: Vec3, size: Vec3) {
        commands.spawn((
            self.block(name, art, center, size),
            ChildOf(self.parent),
            CameraObstacle,
        ));
    }

    fn spawn_pickup(
        &self,
        commands: &mut Commands,
        name: &str,
        center: Vec2,
        pos: UVec2,
        kind: ItemKind,
    ) {
        let ground = CollisionMap::center(pos);
        let root = commands
            .spawn((
                Name::new(name.to_owned()),
                Pickable::new(kind, ground),
                MapTile(pos),
                Transform::from_xyz(center.x, LAND_TOP, center.y),
                Visibility::default(),
                ChildOf(self.parent),
            ))
            .id();
        let parts: &[(BlockArt, [f32; 3], [f32; 3])] = match kind {
            ItemKind::Herb => &[
                (BlockArt::Stem, [0.0, 0.14, 0.0], [0.10, 0.28, 0.10]),
                (BlockArt::Leaves, [-0.09, 0.20, 0.0], [0.18, 0.08, 0.14]),
                (BlockArt::Leaves, [0.08, 0.28, 0.0], [0.17, 0.08, 0.13]),
            ],
            ItemKind::Flower => &[
                (BlockArt::Stem, [0.0, 0.20, 0.0], [0.08, 0.40, 0.08]),
                (BlockArt::Leaves, [0.09, 0.23, 0.0], [0.14, 0.08, 0.12]),
                (BlockArt::Flower, [0.0, 0.43, 0.0], [0.25, 0.12, 0.25]),
            ],
            ItemKind::Mushroom => &[
                (BlockArt::Bark, [0.0, 0.10, 0.0], [0.12, 0.20, 0.12]),
                (BlockArt::Flower, [0.0, 0.22, 0.0], [0.33, 0.13, 0.33]),
                (BlockArt::Flower, [0.0, 0.31, 0.0], [0.21, 0.07, 0.21]),
            ],
            ItemKind::Fern => &[
                (BlockArt::Stem, [0.0, 0.13, 0.0], [0.09, 0.26, 0.09]),
                (BlockArt::Leaves, [0.0, 0.21, 0.0], [0.38, 0.08, 0.12]),
                (BlockArt::Leaves, [0.0, 0.29, 0.0], [0.25, 0.08, 0.12]),
                (BlockArt::Leaves, [0.0, 0.37, 0.0], [0.13, 0.08, 0.12]),
            ],
        };
        for &(art, offset, size) in parts {
            commands.spawn((
                self.block("Pickup detail", art, offset.into(), size.into()),
                ChildOf(root),
            ));
        }
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
            TileKind::Stump => part(name, BlockArt::Bark, 0.0, 0.0, 0.0, 0.46, 0.29, 0.46),
            _ => unreachable!("expected a decoration tile"),
        }
    }
}

pub(super) fn spawn_map_3d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    terrain: Res<TerrainBuild>,
) {
    let mut surfaces = vec![TileKind::Dirt; COUNT];
    let mut props = vec![None; COUNT];
    // Each column shows only the highest surface; props are independent.
    for (pos, _, instance) in terrain.cells() {
        let Some(asset) = terrain.definitions[instance.model_index].first() else {
            continue;
        };
        let name = asset.sprite_name();
        let kind = TileKind::from_sprite(name);
        let cell = CollisionMap::index(pos.as_ivec2()).unwrap();
        match kind {
            TileKind::Dirt | TileKind::Grass | TileKind::YellowGrass | TileKind::Water => {
                surfaces[cell] = kind
            }
            _ => props[cell] = Some((kind, name, asset.pickup)),
        }
    }

    let palette = BlockPalette {
        mesh: meshes.add(block_mesh()),
        materials: BlockArt::ALL.map(|art| materials.add(art.material(&mut images))),
        parent: commands
            .spawn((
                TerrainRoot,
                Name::new("Generated terrain (3D)"),
                Transform::default(),
                Visibility::default(),
            ))
            .id(),
    };
    for (i, (surface, prop)) in surfaces.into_iter().zip(props).enumerate() {
        let pos = CollisionMap::coords(i);
        let ground = CollisionMap::center(pos) / TILE_SIZE;
        let center = Vec2::new(ground.x, -ground.y);
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
                let shallow = terrain.collision.surface(pos) == Tile::Shore;
                layer(
                    "water",
                    BlockArt::Water,
                    LAKE_BED,
                    if shallow { LAND_TOP - 0.06 } else { WATER_TOP },
                );
            }
            _ => unreachable!("expected a terrain tile"),
        }
        if let Some((kind, name, pickup)) = prop {
            if let Some(item) = pickup {
                palette.spawn_pickup(&mut commands, name, center, pos, item);
            } else {
                palette.spawn_prop(&mut commands, kind, name, center);
            }
        }
    }

    commands.insert_resource(ClearColor(Color::srgb(0.60, 0.76, 0.88)));
    commands.insert_resource(GlobalAmbientLight {
        brightness: 170.0,
        ..default()
    });
    commands.spawn((
        ChildOf(palette.parent),
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
        let (definitions, _, _) = super::super::rules::build_world();
        let mut seen = [false; 8];
        for asset in definitions.iter().filter_map(|assets| assets.first()) {
            seen[TileKind::from_sprite(asset.sprite_name()) as usize] = true;
        }
        assert!(seen.into_iter().all(|present| present));
    }

    #[test]
    fn spawns_centered_voxels_with_textures_and_recessed_water() {
        let mut app = App::new();
        app.insert_resource(super::super::generate::build_terrain(42).unwrap())
            .init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<Image>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, spawn_map_3d);
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
            .filter(|(name, _, _, t)| {
                name.as_str() != "Pickup detail" && t.translation.y - t.scale.y * 0.5 == 0.0
            })
            .collect();
        assert_eq!(ground.len(), COUNT);
        let sum: Vec3 = ground.iter().map(|(_, _, _, t)| t.translation).sum();
        assert_eq!((sum.x, sum.z), (0.0, 0.0));

        let beds = ground
            .iter()
            .map(|b| ((b.3.translation.x as i32, b.3.translation.z as i32), b.3))
            .collect::<std::collections::HashMap<_, _>>();
        let top = |t: &Transform| t.translation.y + t.scale.y * 0.5;
        let water: Vec<_> = blocks
            .iter()
            .filter(|(name, _, _, _)| name.as_str() == "water")
            .collect();
        assert!(!water.is_empty());
        for (_, _, _, t) in water {
            assert!(top(t) < LAND_TOP);
            let bed = beds[&(t.translation.x as i32, t.translation.z as i32)];
            assert!((top(bed) - (t.translation.y - t.scale.y * 0.5)).abs() < 1e-6);
        }
    }
}
