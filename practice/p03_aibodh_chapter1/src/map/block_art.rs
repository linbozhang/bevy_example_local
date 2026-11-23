//! 原创 16×16 像素材质：每张图集包含侧面、顶面、底面三个格子。
use bevy::{
    asset::RenderAssetUsages,
    image::ImageSampler,
    mesh::VertexAttributeValues,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

const SIDE: usize = 16;
const WIDTH: usize = SIDE * 3;

#[derive(Clone, Copy)]
#[repr(usize)]
pub(super) enum BlockArt {
    Dirt,
    Grass,
    DryGrass,
    Water,
    Bark,
    Leaves,
    Stone,
    Stem,
    Flower,
}

impl BlockArt {
    pub(super) const ALL: [Self; 9] = [
        Self::Dirt,
        Self::Grass,
        Self::DryGrass,
        Self::Water,
        Self::Bark,
        Self::Leaves,
        Self::Stone,
        Self::Stem,
        Self::Flower,
    ];

    pub(super) fn material(self, images: &mut Assets<Image>) -> StandardMaterial {
        let mut pixels = Vec::with_capacity(WIDTH * SIDE * 4);
        for y in 0..SIDE {
            for u in 0..WIDTH {
                let [r, g, b] = self.pixel(u / SIDE, u % SIDE, y);
                pixels.extend([r, g, b, 255]);
            }
        }
        let mut image = Image::new(
            Extent3d {
                width: WIDTH as u32,
                height: SIDE as u32,
                depth_or_array_layers: 1,
            },
            TextureDimension::D2,
            pixels,
            TextureFormat::Rgba8UnormSrgb,
            RenderAssetUsages::default(),
        );
        image.sampler = ImageSampler::nearest();
        StandardMaterial {
            base_color_texture: Some(images.add(image)),
            perceptual_roughness: if matches!(self, Self::Water) {
                0.48
            } else {
                0.95
            },
            reflectance: if matches!(self, Self::Water) {
                0.45
            } else {
                0.2
            },
            ..default()
        }
    }

    fn pixel(self, face: usize, x: usize, y: usize) -> [u8; 3] {
        let fine = speckle(x, y);
        let patch = speckle(x / 3 + 17, y / 3);
        let dirt = || {
            shade(
                if fine > 8 && patch < 0 {
                    [135, 104, 79]
                } else {
                    [123, 83, 51]
                },
                fine + patch / 2,
            )
        };
        match self {
            Self::Dirt => dirt(),
            Self::Grass | Self::DryGrass => {
                if face == 1 || (face == 0 && y < 3 + (speckle(x, 19) + 12) as usize / 6) {
                    let base = if matches!(self, Self::Grass) {
                        [94, 150, 54]
                    } else {
                        [177, 149, 59]
                    };
                    shade(base, fine / 2 + patch)
                } else {
                    dirt()
                }
            }
            Self::Water => {
                let ripple = (y + 2 * (x / 5)) % 8 == 0 && (x + 3 * y) % 13 < 7;
                shade(
                    if ripple {
                        [78, 155, 218]
                    } else {
                        [40, 111, 181]
                    },
                    patch / 2 + fine / 4,
                )
            }
            Self::Bark if face != 0 => {
                let ring = x.abs_diff(7).max(y.abs_diff(7));
                let wood = if ring >= 7 {
                    [91, 59, 35]
                } else if ring % 3 == 0 {
                    [141, 96, 51]
                } else {
                    [174, 129, 74]
                };
                shade(wood, fine / 2)
            }
            Self::Bark => shade([110, 72, 41], 2 * speckle(x, y / 8) + fine / 3),
            Self::Leaves => shade([62, 119, 48], 2 * patch + fine),
            Self::Stone => shade([114, 120, 121], patch + fine / 2),
            Self::Stem => shade([73, 125, 49], fine),
            Self::Flower => shade([221, 129, 59], fine + patch),
        }
    }
}

fn speckle(x: usize, y: usize) -> i16 {
    let mut bits =
        (x as u32 + 31).wrapping_mul(0x9e37_79b9) ^ (y as u32 + 17).wrapping_mul(0x85eb_ca6b);
    bits = (bits ^ (bits >> 16)).wrapping_mul(0x7feb_352d);
    ((bits ^ (bits >> 15)) % 25) as i16 - 12
}

fn shade(rgb: [u8; 3], delta: i16) -> [u8; 3] {
    rgb.map(|channel| (i16::from(channel) + delta).clamp(0, 255) as u8)
}

pub(super) fn block_mesh() -> Mesh {
    let mut mesh = Cuboid::new(1.0, 1.0, 1.0).mesh().build();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        unreachable!("cuboid positions must be float3");
    };
    let Some(VertexAttributeValues::Float32x3(normals)) = mesh.attribute(Mesh::ATTRIBUTE_NORMAL)
    else {
        unreachable!("cuboid normals must be float3");
    };
    let uvs: Vec<[f32; 2]> = positions
        .iter()
        .zip(normals)
        .map(|(&[x, y, z], &[nx, ny, _])| {
            let (slot, u, v) = if ny > 0.0 {
                (1, x + 0.5, z + 0.5)
            } else if ny < 0.0 {
                (2, x + 0.5, z + 0.5)
            } else {
                // 四个侧面都从 Y+ 向下取图，保证草沿不会跑到方块底部。
                (0, if nx != 0.0 { z + 0.5 } else { x + 0.5 }, 0.5 - y)
            };
            // UV 两端取首尾像素的中心，避免采样串到相邻图集格。
            let span = (SIDE - 1) as f32;
            [
                ((slot * SIDE) as f32 + 0.5 + u * span) / WIDTH as f32,
                (0.5 + v * span) / SIDE as f32,
            ]
        })
        .collect();
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grass_atlas_has_green_top_and_dirt_bottom() {
        for x in 0..SIDE {
            assert_eq!(
                BlockArt::Grass.pixel(0, x, 0),
                BlockArt::Grass.pixel(1, x, 0)
            );
            assert_eq!(
                BlockArt::Grass.pixel(0, x, 15),
                BlockArt::Dirt.pixel(0, x, 15)
            );
            assert_eq!(
                BlockArt::Grass.pixel(2, x, 0),
                BlockArt::Dirt.pixel(0, x, 0)
            );
        }
    }
}
