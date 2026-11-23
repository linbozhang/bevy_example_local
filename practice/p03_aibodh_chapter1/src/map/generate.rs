use bevy::prelude::*;
use bevy_procedural_tilemaps::{prelude::*, spawner::spawn_node};

use super::{
    assets::{SpawnableAsset, load_assets, prepare_tilemap_handles},
    rules::build_world,
};

pub const GRID_X: u32 = 25;
pub const GRID_Y: u32 = 18;
pub const GRID_Z: u32 = 5;
pub const TILE_SIZE: f32 = 32.0;
// 固定种子便于教学复现；换成其他数字即可得到另一张地图。
pub const MAP_SEED: u64 = 42;
// 玩家高于地形及装饰层；本章不实现人物/树木的动态遮挡。
pub const PLAYER_Z: f32 = 10.0;

pub type TerrainGenerator = Generator<Cartesian3D, CartesianGrid<Cartesian3D>>;

pub fn create_generator(seed: u64) -> (TerrainGenerator, Vec<Vec<SpawnableAsset>>) {
    let (definitions, models, sockets) = build_world();
    // 装饰层的第一个模型是空白；用外围空白约束防止边界生成半棵树。
    let props_void = definitions
        .iter()
        .rposition(Vec::is_empty)
        .expect("props void model");
    let rules = RulesBuilder::new_cartesian_3d(models, sockets)
        .with_rotation_axis(Direction::ZForward)
        .build()
        .expect("terrain socket rules must be valid");
    let grid = CartesianGrid::new_cartesian_3d(GRID_X, GRID_Y, GRID_Z, false, false, false);
    let mut borders = Vec::new();
    let void_variant = rules
        .variant_index(props_void, ModelRotation::Rot0)
        .unwrap();
    for y in 0..GRID_Y {
        for x in 0..GRID_X {
            if x == 0 || y == 0 || x == GRID_X - 1 || y == GRID_Y - 1 {
                borders.push((grid.index_from_coords(x, y, GRID_Z - 1), void_variant));
            }
        }
    }
    let generator = GeneratorBuilder::new()
        .with_rules(rules)
        .with_grid(grid)
        .with_initial_nodes_raw(borders)
        .with_rng(RngMode::Seeded(seed))
        .with_max_retry_count(50)
        .with_node_heuristic(NodeSelectionHeuristic::MinimumRemainingValue)
        .with_model_heuristic(ModelSelectionHeuristic::WeightedProbability)
        .build()
        .expect("terrain generator configuration must be valid");
    (generator, definitions)
}

pub fn setup_map(
    mut commands: Commands,
    server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let (mut generator, definitions) = create_generator(MAP_SEED);
    // 小地图在 Startup 同步生成：达到重试上限明确失败，不逐帧无限重试。
    let (info, data) = generator
        .generate_grid()
        .expect("WFC exhausted its retry budget");
    info!(
        "Generated {} terrain cells with seed {MAP_SEED}: {info:?}",
        data.iter().count()
    );
    let handles = prepare_tilemap_handles(&server, &mut layouts, "tile_layers", "tilemap.png");
    let spawner = NodesSpawner::new(
        load_assets(&handles, definitions),
        Vec3::new(TILE_SIZE, TILE_SIZE, 1.0),
        Vec3::ONE,
    );
    // 关闭按 y 加 z：避免低层前景盖住高层背景地形。
    let parent = commands
        .spawn((
            Name::new("Generated terrain"),
            Transform::from_xyz(
                -TILE_SIZE * GRID_X as f32 / 2.0,
                -TILE_SIZE * GRID_Y as f32 / 2.0,
                0.0,
            ),
            Visibility::default(),
        ))
        .id();
    for (index, instance) in data.iter().enumerate() {
        spawn_node(
            &mut commands,
            parent,
            generator.grid(),
            &spawner,
            instance,
            index,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::super::tilemap::TILEMAP;
    use super::*;

    #[test]
    fn atlas_metadata_matches_image_and_all_models_have_known_assets() {
        let png = include_bytes!("../assets/tile_layers/tilemap.png");
        assert_eq!(
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            TILEMAP.atlas_width
        );
        assert_eq!(
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
            TILEMAP.atlas_height
        );
        let mut names = std::collections::HashSet::new();
        for (i, sprite) in TILEMAP.sprites.iter().enumerate() {
            assert!(names.insert(sprite.name));
            let rect = TILEMAP.sprite_rect(i);
            assert!(rect.max.x <= TILEMAP.atlas_width && rect.max.y <= TILEMAP.atlas_height);
        }
        let (definitions, models, _) = build_world();
        assert_eq!(definitions.len(), models.models_count());
        for asset in definitions.iter().flatten() {
            assert!(TILEMAP.sprite_index(asset.sprite_name()).is_some());
        }
    }

    #[test]
    fn seeded_maps_obey_layer_and_land_only_prop_rules() {
        let (definitions, _, _) = build_world();
        let mut water_count = 0;
        let mut prop_count = 0;
        for seed in [42, 7, 123] {
            let (mut generator, _) = create_generator(seed);
            let (_, data) = generator
                .generate_grid()
                .expect("seed must produce a map within budget");
            assert_eq!(data.iter().count(), (GRID_X * GRID_Y * GRID_Z) as usize);
            let (mut repeat, _) = create_generator(seed);
            assert_eq!(
                data.iter().collect::<Vec<_>>(),
                repeat.generate_grid().unwrap().1.iter().collect::<Vec<_>>()
            );
            let cells: Vec<_> = data.iter().collect();
            for (index, instance) in data.iter().enumerate() {
                let pos = generator.grid().pos_from_index(index);
                let assets = &definitions[instance.model_index];
                for asset in assets {
                    let name = asset.sprite_name();
                    let expected_layer = if name == "dirt" {
                        0
                    } else if name.starts_with("green_grass") {
                        1
                    } else if name.starts_with("yellow_grass") {
                        2
                    } else if name.starts_with("water") {
                        3
                    } else {
                        4
                    };
                    assert_eq!(pos.z, expected_layer, "{name} at {pos:?}");
                    if expected_layer == 3 {
                        water_count += 1;
                    }
                }
                if pos.z == 0 {
                    assert_eq!(assets.len(), 1);
                    assert_eq!(assets[0].sprite_name(), "dirt");
                }
                if pos.z == 4 && !assets.is_empty() {
                    prop_count += 1;
                    assert!(pos.x > 0 && pos.x < GRID_X - 1 && pos.y > 0 && pos.y < GRID_Y - 1);
                    for asset in assets {
                        let name = asset.sprite_name();
                        if name == "small_tree_bottom"
                            || (name.starts_with("big_tree")
                                && (name.ends_with("_bl") || name.ends_with("_br")))
                        {
                            assert!(
                                definitions[cells[index + GRID_X as usize].model_index].is_empty(),
                                "canopy cell occupied by another prop"
                            );
                        }
                        if name.starts_with("big_tree") && name.ends_with("_bl") {
                            let right = &definitions[cells[index + 1].model_index];
                            let expected = name.replace("_bl", "_br");
                            assert!(right.iter().any(|part| part.sprite_name() == expected));
                        }
                        if name.starts_with("big_tree") && name.ends_with("_br") {
                            let left = &definitions[cells[index - 1].model_index];
                            let expected = name.replace("_br", "_bl");
                            assert!(left.iter().any(|part| part.sprite_name() == expected));
                        }
                    }
                    // index 的 z 步长为整个 xy 平面，下面一层必须是透明陆地。
                    let below = &definitions[cells[index - (GRID_X * GRID_Y) as usize].model_index];
                    assert!(below.is_empty(), "prop placed over water");
                }
            }
        }
        assert!(water_count > 0, "test seeds should exercise water rules");
        assert!(prop_count > 0, "test seeds should exercise prop rules");
    }
}
