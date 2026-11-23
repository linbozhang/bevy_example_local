//! Chapter 9 overlap stitching: each chunk pins all five layers of its west
//! and south edges to its already collapsed neighbors. Renderers and collision
//! consume the same deduplicated world grid, never the individual chunk grids.
use std::sync::Arc;

use bevy::prelude::*;
use bevy_procedural_tilemaps::{
    prelude::*,
    proc_gen::generator::{model::ModelInstance, rules::Rules},
};

use super::{assets::SpawnableAsset, collision::CollisionMap, rules::build_world};

const GRID_X: u32 = 25;
const GRID_Y: u32 = 18;
pub(super) const GRID_Z: u32 = 5;
const CHUNKS: UVec2 = UVec2::new(10, 10);
const STRIDE: UVec2 = UVec2::new(GRID_X - 1, GRID_Y - 1);
pub(super) const SIZE: UVec2 = UVec2::new(CHUNKS.x * STRIDE.x + 1, CHUNKS.y * STRIDE.y + 1);
pub(super) const COUNT: usize = (SIZE.x * SIZE.y) as usize;
pub const TILE_SIZE: f32 = 64.0;
pub(crate) const TILEMAP_IMAGE: &str = "tile_layers/tilemap.png";
pub const MAP_SEED: u64 = 42;
const CHUNK_RETRIES: u32 = 8;
const MAX_BACKTRACKS: u32 = 128;

// Ground layers occupy Z < 4; props and players share the next layer.
pub(crate) fn depth_from_feet(y: f32) -> f32 {
    5.0 - (y / (TILE_SIZE * SIZE.y as f32) + 0.5).clamp(0.0, 1.0)
}

type TerrainGrid = CartesianGrid<Cartesian3D>;
type TerrainRules = Arc<Rules<Cartesian3D>>;
type Pin = (usize, usize);
type Definitions = Vec<Vec<SpawnableAsset>>;

#[derive(Resource)]
pub(super) struct TerrainBuild {
    pub seed: u64,
    pub collision: CollisionMap,
    pub definitions: Definitions,
    cells: Vec<ModelInstance>,
}

impl TerrainBuild {
    pub fn cells(&self) -> impl Iterator<Item = (UVec2, u32, &ModelInstance)> + '_ {
        cells(&self.cells, SIZE)
    }
}

fn cells(
    data: &[ModelInstance],
    size: UVec2,
) -> impl Iterator<Item = (UVec2, u32, &ModelInstance)> {
    data.iter().enumerate().map(move |(i, instance)| {
        let i = i as u32;
        (
            UVec2::new(i % size.x, i / size.x % size.y),
            i / (size.x * size.y),
            instance,
        )
    })
}

fn terrain_rules() -> (TerrainRules, usize, Definitions) {
    let (definitions, models, sockets) = build_world();
    let props_void = definitions
        .iter()
        .rposition(Vec::is_empty)
        .expect("props void model");
    let rules = RulesBuilder::new_cartesian_3d(models, sockets)
        .with_rotation_axis(Direction::ZForward)
        .build()
        .expect("terrain socket rules must be valid");
    let props_void = rules
        .variant_index(props_void, ModelRotation::Rot0)
        .unwrap();
    (Arc::new(rules), props_void, definitions)
}

fn chunk_grid() -> TerrainGrid {
    CartesianGrid::new_cartesian_3d(GRID_X, GRID_Y, GRID_Z, false, false, false)
}

fn chunk_pins(
    index: usize,
    chunks: UVec2,
    grid: &TerrainGrid,
    rules: &TerrainRules,
    props_void: usize,
    done: &[Vec<ModelInstance>],
) -> Vec<Pin> {
    let cx = index as u32 % chunks.x;
    let cy = index as u32 / chunks.x;
    let left = (cx > 0).then(|| &done[index - 1]);
    let bottom = (cy > 0).then(|| &done[index - chunks.x as usize]);
    let mut pins = Vec::new();
    for y in 0..GRID_Y {
        for x in 0..GRID_X {
            let exterior = (cx == 0 && x == 0)
                || (cy == 0 && y == 0)
                || (cx == chunks.x - 1 && x == GRID_X - 1)
                || (cy == chunks.y - 1 && y == GRID_Y - 1);
            // At the southwest corner, both neighbors already agree via
            // the southwest chunk. Pin that cell only once.
            let neighbor = if x == 0
                && let Some(left) = left
            {
                Some((left, GRID_X - 1, y))
            } else if y == 0
                && let Some(bottom) = bottom
            {
                Some((bottom, x, GRID_Y - 1))
            } else {
                None
            };
            if let Some((neighbor, nx, ny)) = neighbor {
                for z in 0..GRID_Z {
                    let model = neighbor[grid.index_from_coords(nx, ny, z)];
                    pins.push((
                        grid.index_from_coords(x, y, z),
                        rules
                            .variant_index(model.model_index, model.rotation)
                            .unwrap(),
                    ));
                }
            } else if exterior {
                // Only the outside of the entire map must be prop-free. Internal
                // edges may contain trees, including bases/canopies spanning a seam.
                pins.push((grid.index_from_coords(x, y, GRID_Z - 1), props_void));
            }
        }
    }
    pins
}

fn solve_chunk(
    grid: &TerrainGrid,
    rules: &TerrainRules,
    pins: &[Pin],
    seed: u64,
) -> Result<Vec<ModelInstance>, String> {
    let mut generator = GeneratorBuilder::new()
        .with_shared_rules(rules.clone())
        .with_grid(grid.clone())
        .with_initial_nodes_raw(pins.to_vec())
        .with_rng(RngMode::Seeded(seed))
        .with_max_retry_count(CHUNK_RETRIES)
        .with_node_heuristic(NodeSelectionHeuristic::MinimumRemainingValue)
        .with_model_heuristic(ModelSelectionHeuristic::WeightedProbability)
        .build()
        .map_err(|error| error.to_string())?;
    let (_, data) = generator
        .generate_grid()
        .map_err(|error| error.to_string())?;
    Ok(data.iter().copied().collect())
}

// SplitMix64: a fixed, wrapping mixer keeps per-attempt seeds independent of
// task scheduling and RNG-library changes outside the WFC solver itself.
fn attempt_seed(seed: u64, attempt: u64) -> u64 {
    let mut z = seed.wrapping_add(attempt.wrapping_add(1).wrapping_mul(0x9e3779b97f4a7c15));
    z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
    z ^ (z >> 31)
}

fn stitch(
    seed: u64,
    chunks: UVec2,
    rules: &TerrainRules,
    props_void: usize,
) -> Result<Vec<ModelInstance>, String> {
    let grid = chunk_grid();
    let count = (chunks.x * chunks.y) as usize;
    let mut done = Vec::with_capacity(count);
    let mut backtracks = 0;
    let mut attempt = 0;
    while done.len() < count {
        let i = done.len();
        let x = i % chunks.x as usize;
        let y = i / chunks.x as usize;
        let pins = chunk_pins(i, chunks, &grid, rules, props_void, &done);
        let result = solve_chunk(&grid, rules, &pins, attempt_seed(seed, attempt));
        attempt += 1;
        match result {
            Ok(chunk) => done.push(chunk),
            Err(error) if backtracks >= MAX_BACKTRACKS => {
                return Err(format!(
                    "Terrain seed {seed}: chunk ({x}, {y}) failed after {backtracks} backtracks: {error}"
                ));
            }
            Err(_) => {
                // Reopen the root of the conflicting corner (or the one pinned
                // neighbor along the first row/column) rather than relaxing a seam.
                let rollback = usize::from(x > 0) + usize::from(y > 0) * chunks.x as usize;
                done.truncate(i - rollback);
                backtracks += 1;
            }
        }
    }

    let size = chunks * STRIDE + UVec2::ONE;
    let mut stitched = vec![None; (size.x * size.y * GRID_Z) as usize];
    for (i, chunk) in done.iter().enumerate() {
        let offset = UVec2::new(i as u32 % chunks.x, i as u32 / chunks.x) * STRIDE;
        for (local, z, &model) in cells(chunk, UVec2::new(GRID_X, GRID_Y)) {
            let pos = local + offset;
            let slot = &mut stitched[((z * size.y + pos.y) * size.x + pos.x) as usize];
            if slot
                .replace(model)
                .is_some_and(|existing| existing != model)
            {
                return Err(format!(
                    "Terrain seed {seed}: mismatching seam at {pos:?}, layer {z}"
                ));
            }
        }
    }
    Ok(stitched
        .into_iter()
        .map(|cell| cell.expect("covered by a chunk"))
        .collect())
}

/// A finite WFC detail field in voxel coordinates. Macro terrain owns heights,
/// water and villages; WFC supplies coherent ground variants and plant sites.
pub(crate) struct VoxelDetails {
    pub dry_grass: Vec<bool>,
    pub plants: Vec<(i32, i32, crate::inventory::ItemKind)>,
}

pub(crate) fn voxel_details(seed: u64) -> Result<VoxelDetails, String> {
    let (rules, props_void, definitions) = terrain_rules();
    let data = stitch(seed, CHUNKS, &rules, props_void)?;
    let mut result = VoxelDetails {
        dry_grass: vec![false; COUNT],
        plants: Vec::new(),
    };
    for (p, layer, model) in cells(&data, SIZE) {
        let Some(asset) = definitions[model.model_index].first() else {
            continue;
        };
        if layer == 2 && asset.sprite_name().starts_with("yellow_grass") {
            result.dry_grass[(p.y * SIZE.x + p.x) as usize] = true;
        }
        if let Some(kind) = asset.pickup {
            result.plants.push((
                p.x as i32 - SIZE.x as i32 / 2,
                SIZE.y as i32 / 2 - p.y as i32,
                kind,
            ));
        }
    }
    Ok(result)
}

impl VoxelDetails {
    pub fn dry_at(&self, x: i32, z: i32) -> bool {
        let p = IVec2::new(x + SIZE.x as i32 / 2, SIZE.y as i32 / 2 - z);
        p.cmpge(IVec2::ZERO).all()
            && p.cmplt(SIZE.as_ivec2()).all()
            && self.dry_grass[(p.y as u32 * SIZE.x + p.x as u32) as usize]
    }
}

pub(super) fn build_terrain(seed: u64) -> Result<TerrainBuild, String> {
    let (rules, props_void, definitions) = terrain_rules();
    let data = stitch(seed, CHUNKS, &rules, props_void)?;
    let collision = CollisionMap::from_tiles(cells(&data, SIZE).filter_map(|(pos, _, model)| {
        let asset = definitions[model.model_index].first()?;
        Some((pos.x, pos.y, asset.sprite_name()))
    }));
    Ok(TerrainBuild {
        seed,
        collision,
        definitions,
        cells: data,
    })
}

#[cfg(test)]
mod tests {
    use super::super::tilemap::TILEMAP;
    use super::*;

    #[test]
    fn stitched_chunks_are_repeatable_and_keep_props_aligned_across_seams() {
        let generate = |seed| {
            let (rules, props_void, definitions) = terrain_rules();
            let chunks = UVec2::splat(2);
            (
                stitch(seed, chunks, &rules, props_void).unwrap(),
                definitions,
                chunks * STRIDE + UVec2::ONE,
            )
        };
        let (data, definitions, size) = generate(MAP_SEED);
        assert_eq!(data.len(), (size.x * size.y * GRID_Z) as usize);
        assert_eq!(data, generate(MAP_SEED).0);
        assert_ne!(data, generate(7).0);

        let props = |pos: UVec2| {
            &definitions
                [data[(((GRID_Z - 1) * size.y + pos.y) * size.x + pos.x) as usize].model_index]
        };
        for (pos, z, _) in cells(&data, size) {
            if z != GRID_Z - 1 {
                continue;
            }
            let Some(base) = props(pos).first() else {
                continue;
            };
            assert!(pos.cmpgt(UVec2::ZERO).all() && pos.cmplt(size - UVec2::ONE).all());
            let name = base.sprite_name();
            if name.starts_with("small_tree") || name.starts_with("big_tree") {
                assert!(
                    props(pos + UVec2::Y).is_empty(),
                    "blocked canopy at {pos:?}"
                );
            }
            if let Some(stem) = name.strip_suffix("_bl") {
                assert_eq!(props(pos + UVec2::X)[0].sprite_name(), format!("{stem}_br"));
            }
            if let Some(stem) = name.strip_suffix("_br") {
                assert_eq!(props(pos - UVec2::X)[0].sprite_name(), format!("{stem}_bl"));
            }
        }
    }

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
            let (rules, props_void, _) = terrain_rules();
            let grid = chunk_grid();
            let pins = chunk_pins(0, UVec2::ONE, &grid, &rules, props_void, &[]);
            let data = solve_chunk(&grid, &rules, &pins, seed).unwrap();
            assert_eq!(data.iter().count(), (GRID_X * GRID_Y * GRID_Z) as usize);
            assert_eq!(data, solve_chunk(&grid, &rules, &pins, seed).unwrap());
            let cells: Vec<_> = data.iter().collect();
            for (index, instance) in data.iter().enumerate() {
                let pos = grid.pos_from_index(index);
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
