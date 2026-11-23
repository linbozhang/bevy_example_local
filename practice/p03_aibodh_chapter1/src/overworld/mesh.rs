//! Build separate draw meshes for solid terrain and translucent water.

use bevy::{
    asset::RenderAssetUsages,
    math::IVec2,
    mesh::{Indices, Mesh, PrimitiveTopology},
};

use super::{CHUNK_SIZE, MAX_Y, MIN_Y, WorldGen, terrain::Block};
const HEIGHT: usize = (MAX_Y - MIN_Y + 1) as usize;
const COLUMN_STRIDE: usize = HEIGHT + 2;
const Z_STRIDE: usize = (CHUNK_SIZE as usize + 2) * COLUMN_STRIDE;

/// Vertices are chunk-local in X/Z and world-space in Y. A missing mesh has no faces.
pub(super) struct ChunkMesh {
    pub(super) opaque: Option<Mesh>,
    /// Use a white base color and `AlphaMode::Blend` to respect vertex alpha.
    pub(super) water: Option<Mesh>,
}

pub(super) fn build(world: &WorldGen, chunk: IVec2) -> ChunkMesh {
    build_with(chunk, |x, z, column| world.fill_column(x, z, column))
}

fn build_with(chunk: IVec2, mut fill_column: impl FnMut(i32, i32, &mut [Block])) -> ChunkMesh {
    let origin = chunk * CHUNK_SIZE;
    // Y is contiguous for the column generator. The air padding at the ends of
    // each column models the finite vertical range. The X/Z halo instead uses
    // real world blocks, even when the neighboring chunk isn't loaded yet.
    let mut blocks = vec![Block::Air; (CHUNK_SIZE as usize + 2) * Z_STRIDE];
    for z in -1..=CHUNK_SIZE {
        for x in -1..=CHUNK_SIZE {
            let start = column_start(x, z);
            fill_column(
                origin.x + x,
                origin.y + z,
                &mut blocks[start..start + HEIGHT],
            );
        }
    }

    let mut opaque = MeshBuffer::default();
    let mut water = MeshBuffer::default();
    for z in 0..CHUNK_SIZE {
        for x in 0..CHUNK_SIZE {
            let start = column_start(x, z);
            for (dy, &block) in blocks[start..start + HEIGHT].iter().enumerate() {
                if block == Block::Air {
                    continue;
                }
                let i = start + dy;
                let y = MIN_Y + dy as i32;
                let is_water = block == Block::Water;
                let mut tint = None;
                for face in &FACES {
                    let neighbor = blocks[i.wrapping_add_signed(face.neighbor)];
                    // The solid mesh owns solid/water boundaries. Water only
                    // faces air, so no interface has two coplanar quads.
                    if neighbor != Block::Air && (is_water || neighbor != Block::Water) {
                        continue;
                    }
                    let mesh = if is_water { &mut water } else { &mut opaque };
                    let tint =
                        *tint.get_or_insert_with(|| variation(origin.x + x, y, origin.y + z));
                    mesh.push_face(
                        [x as f32, y as f32, z as f32],
                        face,
                        palette(block, face.normal[1]),
                        tint,
                    );
                }
            }
        }
    }

    ChunkMesh {
        opaque: opaque.finish(),
        water: water.finish(),
    }
}

/// Offset of the first world Y cell in a padded column.
fn column_start(x: i32, z: i32) -> usize {
    (z + 1) as usize * Z_STRIDE + (x + 1) as usize * COLUMN_STRIDE + 1
}

struct Face {
    neighbor: isize,
    normal: [f32; 3],
    corners: [[f32; 3]; 4],
}

// Corners wind counterclockwise as viewed from outside the voxel.
const FACES: [Face; 6] = [
    Face {
        neighbor: -(COLUMN_STRIDE as isize),
        normal: [-1., 0., 0.],
        corners: [[0., 0., 0.], [0., 0., 1.], [0., 1., 1.], [0., 1., 0.]],
    },
    Face {
        neighbor: COLUMN_STRIDE as isize,
        normal: [1., 0., 0.],
        corners: [[1., 0., 1.], [1., 0., 0.], [1., 1., 0.], [1., 1., 1.]],
    },
    Face {
        neighbor: -1,
        normal: [0., -1., 0.],
        corners: [[0., 0., 0.], [1., 0., 0.], [1., 0., 1.], [0., 0., 1.]],
    },
    Face {
        neighbor: 1,
        normal: [0., 1., 0.],
        corners: [[0., 1., 1.], [1., 1., 1.], [1., 1., 0.], [0., 1., 0.]],
    },
    Face {
        neighbor: -(Z_STRIDE as isize),
        normal: [0., 0., -1.],
        corners: [[1., 0., 0.], [0., 0., 0.], [0., 1., 0.], [1., 1., 0.]],
    },
    Face {
        neighbor: Z_STRIDE as isize,
        normal: [0., 0., 1.],
        corners: [[0., 0., 1.], [1., 0., 1.], [1., 1., 1.], [0., 1., 1.]],
    },
];

/// Linear RGB for Bevy's vertex-color attribute. Glass and leaves use opaque
/// colors so water is the only material that needs a blended pass.
fn palette(block: Block, normal_y: f32) -> [f32; 4] {
    let [r, g, b] = match block {
        Block::Air => unreachable!("air has no faces"),
        Block::Grass if normal_y > 0. => [0.20, 0.46, 0.10],
        Block::Grass if normal_y == 0. => [0.24, 0.32, 0.09],
        Block::DryGrass if normal_y > 0. => [0.39, 0.36, 0.12],
        Block::DryGrass | Block::Grass | Block::Dirt => [0.27, 0.16, 0.08],
        Block::Stone => [0.32, 0.33, 0.34],
        Block::Sand => [0.73, 0.60, 0.34],
        Block::Snow => [0.82, 0.91, 0.95],
        Block::Water => [0.025, 0.19, 0.45],
        Block::Log if normal_y != 0. => [0.49, 0.30, 0.13],
        Block::Log => [0.24, 0.13, 0.055],
        Block::Leaves => [0.075, 0.29, 0.085],
        Block::Planks => [0.56, 0.34, 0.15],
        Block::Path => [0.42, 0.28, 0.115],
        Block::Glass => [0.44, 0.70, 0.79],
    };
    let alpha = if block == Block::Water { 0.72 } else { 1. };
    [r, g, b, alpha]
}

fn variation(x: i32, y: i32, z: i32) -> f32 {
    // Hash world positions, not chunk-local ones, so the color pattern doesn't repeat at seams.
    let mut bits = (x as u32).wrapping_mul(0x9e37_79b1)
        ^ (y as u32).wrapping_mul(0x85eb_ca77)
        ^ (z as u32).wrapping_mul(0xc2b2_ae3d);
    bits ^= bits >> 16;
    bits = bits.wrapping_mul(0x7feb_352d);
    0.92 + ((bits >> 24) as f32 / 255.) * 0.14
}

#[derive(Default)]
struct MeshBuffer {
    positions: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    colors: Vec<[f32; 4]>,
    indices: Vec<u32>,
}

impl MeshBuffer {
    fn push_face(&mut self, [x, y, z]: [f32; 3], face: &Face, [r, g, b, a]: [f32; 4], tint: f32) {
        let i = self.positions.len() as u32;
        self.positions
            .extend(face.corners.map(|[dx, dy, dz]| [x + dx, y + dy, z + dz]));
        self.normals.extend([face.normal; 4]);
        self.colors.extend([[r * tint, g * tint, b * tint, a]; 4]);
        self.indices.extend([i, i + 1, i + 2, i, i + 2, i + 3]);
    }

    fn finish(self) -> Option<Mesh> {
        if self.indices.is_empty() {
            return None;
        }
        // The chunk is regenerated rather than edited in place, so there is no
        // need to keep the CPU vertex buffers after render extraction.
        Some(
            Mesh::new(
                PrimitiveTopology::TriangleList,
                RenderAssetUsages::RENDER_WORLD,
            )
            .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, self.positions)
            .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, self.normals)
            .with_inserted_attribute(Mesh::ATTRIBUTE_COLOR, self.colors)
            .with_inserted_indices(Indices::U32(self.indices)),
        )
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use bevy::{math::Vec3, mesh::VertexAttributeValues};

    use super::*;

    fn from_blocks(chunk: IVec2, cells: &[([i32; 3], Block)]) -> ChunkMesh {
        build_with(chunk, |x, z, column| {
            for &([cx, y, cz], block) in cells {
                if (x, z) == (cx, cz) {
                    column[(y - MIN_Y) as usize] = block;
                }
            }
        })
    }

    fn face_count(mesh: &Mesh) -> usize {
        assert_eq!(mesh.indices().unwrap().len() * 2, mesh.count_vertices() * 3);
        mesh.count_vertices() / 4
    }

    fn positions(mesh: &Mesh) -> &[[f32; 3]] {
        let Some(VertexAttributeValues::Float32x3(values)) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION)
        else {
            panic!("missing vertex positions");
        };
        values
    }

    fn colors(mesh: &Mesh) -> &[[f32; 4]] {
        let Some(VertexAttributeValues::Float32x4(values)) = mesh.attribute(Mesh::ATTRIBUTE_COLOR)
        else {
            panic!("missing vertex colors");
        };
        values
    }

    #[test]
    fn empty_chunk_samples_halo_once_and_produces_no_meshes() {
        let mut samples = HashSet::new();
        let mesh = build_with(IVec2::new(-1, 2), |x, z, column| {
            assert_eq!(column.len(), 133);
            assert!((-17..=0).contains(&x) && (31..=48).contains(&z));
            assert!(samples.insert((x, z)));
        });
        assert_eq!(samples.len(), 18 * 18);
        assert!(mesh.opaque.is_none() && mesh.water.is_none());
    }

    #[test]
    fn one_cube_has_outward_winding_local_xz_and_world_y() {
        let mesh = from_blocks(IVec2::new(-2, 3), &[([-28, -7, 56], Block::Grass)])
            .opaque
            .unwrap();
        assert_eq!(face_count(&mesh), 6);
        let positions = positions(&mesh);
        assert!(positions.iter().all(|&[x, y, z]| {
            (x == 4. || x == 5.) && (y == -7. || y == -6.) && (z == 8. || z == 9.)
        }));
        let Some(VertexAttributeValues::Float32x3(normals)) =
            mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
        else {
            panic!("missing vertex normals");
        };
        let indices: Vec<_> = mesh.indices().unwrap().iter().collect();
        let center = Vec3::new(4.5, -6.5, 8.5);
        for triangle in indices.chunks_exact(3) {
            let p = Vec3::from_array(positions[triangle[0]]);
            let q = Vec3::from_array(positions[triangle[1]]);
            let r = Vec3::from_array(positions[triangle[2]]);
            let normal = (q - p).cross(r - p);
            assert!(normal.dot(p - center) > 0.);
            for &i in triangle {
                assert_eq!(Vec3::from_array(normals[i]), normal);
            }
        }
        let colors = colors(&mesh);
        let top = normals.iter().position(|&n| n == [0., 1., 0.]).unwrap();
        let bottom = normals.iter().position(|&n| n == [0., -1., 0.]).unwrap();
        assert!(colors[top][1] > colors[bottom][1]);
    }

    #[test]
    fn matching_solid_faces_are_culled_inside_and_across_chunks() {
        let cells = [
            ([-1, 5, -1], Block::Stone),
            ([0, 5, -1], Block::Dirt),
            ([0, 5, 0], Block::Sand),
            ([0, 6, 0], Block::Stone),
        ];
        for (chunk, expected_faces) in [
            (IVec2::new(-1, -1), 5),
            (IVec2::new(0, -1), 4),
            (IVec2::ZERO, 9),
        ] {
            let mesh = from_blocks(chunk, &cells).opaque.unwrap();
            assert_eq!(face_count(&mesh), expected_faces, "chunk {chunk}");
        }
    }

    #[test]
    fn water_faces_only_air_while_solids_face_water() {
        let mesh = from_blocks(
            IVec2::ZERO,
            &[
                ([0, 0, 0], Block::Stone),
                ([1, 0, 0], Block::Water),
                ([2, 0, 0], Block::Water),
            ],
        );
        let opaque = mesh.opaque.unwrap();
        let water = mesh.water.unwrap();
        assert_eq!(face_count(&opaque), 6);
        assert_eq!(face_count(&water), 9);
        assert!(colors(&opaque).iter().all(|c| c[3] == 1.));
        assert!(colors(&water).iter().all(|c| 0. < c[3] && c[3] < 1.));
    }

    #[test]
    fn water_continues_across_seams_and_vertical_limits_border_air() {
        let cells = [
            ([8, MIN_Y, -1], Block::Water),
            ([8, MIN_Y, 0], Block::Water),
            ([9, MAX_Y, 1], Block::Snow),
        ];
        let mesh = from_blocks(IVec2::ZERO, &cells);
        let water = mesh.water.unwrap();
        let opaque = mesh.opaque.unwrap();
        assert_eq!(face_count(&water), 5);
        assert_eq!(face_count(&opaque), 6);
        assert!(positions(&water).iter().any(|p| p[1] == MIN_Y as f32));
        assert!(
            positions(&opaque)
                .iter()
                .any(|p| p[1] == (MAX_Y + 1) as f32)
        );
    }
}
