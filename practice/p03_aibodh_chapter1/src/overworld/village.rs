use super::terrain::{Biome, Block, SEA_LEVEL, Terrain};

const FOOTPRINT_RADIUS: i32 = 16;
const MAX_RELIEF: i32 = 4;
const CLEARANCE: i32 = 9;
const REGION_SIZE: i32 = 128;
// Keep adjacent regions' candidates apart even when both jitter toward an edge.
// Also keep all regular candidates away from the reserved origin footprint.
const REGION_MARGIN: i32 = 40;
const REGION_RADIUS: i32 = 2;

pub(super) struct Villages {
    sites: Vec<Site>,
}

impl Villages {
    pub fn new(seed: u64, terrain: &Terrain) -> Self {
        let mut sites = Vec::new();
        let mut add_site = |x, z| {
            if let Some(y) = platform_height(x, z, |x, z| {
                let column = terrain.column(x, z);
                column.water.is_none().then_some(column.height)
            }) {
                sites.push(Site::new(x, y, z, seed, terrain.column(x, z).biome));
            }
        };

        // Terrain generation can guarantee the showcase village by reserving a
        // flat, dry, above-sea-level square of radius FOOTPRINT_RADIUS here.
        add_site(0, 0);
        for rz in -REGION_RADIUS..=REGION_RADIUS {
            for rx in -REGION_RADIUS..=REGION_RADIUS {
                let (x, z) = candidate(seed, rx, rz);
                add_site(x, z);
            }
        }
        Self { sites }
    }

    pub fn spawn(&self) -> (i32, i32) {
        // The reserved origin is added first; otherwise use the first valid
        // regular village, or let the caller fall back to terrain at the origin.
        self.sites.first().map_or((0, 0), |site| (site.x, site.z))
    }

    pub(super) fn floor(&self, x: i32, z: i32) -> Option<i32> {
        self.sites
            .iter()
            .find(|site| {
                (site.x - x).abs() <= FOOTPRINT_RADIUS && (site.z - z).abs() <= FOOTPRINT_RADIUS
            })
            .map(|site| site.y)
    }

    /// `Some(Air)` cuts the terrain or clears decoration; `None` leaves it alone.
    pub fn block(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        self.sites.iter().find_map(|site| site.block(x, y, z))
    }
}

fn platform_height(
    x: i32,
    z: i32,
    mut dry_height: impl FnMut(i32, i32) -> Option<i32>,
) -> Option<i32> {
    let (mut low, mut high) = (i32::MAX, i32::MIN);
    for dz in -FOOTPRINT_RADIUS..=FOOTPRINT_RADIUS {
        for dx in -FOOTPRINT_RADIUS..=FOOTPRINT_RADIUS {
            let height = dry_height(x + dx, z + dz)?;
            if height <= SEA_LEVEL {
                return None;
            }
            low = low.min(height);
            high = high.max(height);
            if high - low > MAX_RELIEF {
                return None;
            }
        }
    }
    Some(low + (high - low) / 2)
}

fn candidate(seed: u64, rx: i32, rz: i32) -> (i32, i32) {
    let h = coordinate_hash(seed, rx, rz);
    let span = (REGION_SIZE - 2 * REGION_MARGIN) as u64;
    (
        rx * REGION_SIZE + REGION_MARGIN + (h % span) as i32,
        rz * REGION_SIZE + REGION_MARGIN + ((h >> 32) % span) as i32,
    )
}

fn coordinate_hash(seed: u64, x: i32, z: i32) -> u64 {
    let h = seed
        ^ (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
        ^ (z as u64).wrapping_mul(0xD1B5_4A32_D192_ED03);
    let h = (h ^ (h >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    let h = (h ^ (h >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    h ^ (h >> 31)
}

struct Site {
    x: i32,
    y: i32,
    z: i32,
    palette: Palette,
    houses: [House; 4],
}

impl Site {
    fn new(x: i32, y: i32, z: i32, seed: u64, biome: Biome) -> Self {
        let layout = coordinate_hash(seed ^ 0xA881_B4D2_78A9_1733, x, z);
        let houses = std::array::from_fn(|i| {
            let side = if i < 2 { -1 } else { 1 };
            let bits = layout >> (i * 8);
            House {
                x: if i % 2 == 0 { -9 } else { 9 } + (bits % 3) as i32 - 1,
                // Place all doors at |z| = 5, regardless of depth.
                depth: 2 + ((bits >> 2) & 1) as i32,
                width: 2 + ((bits >> 3) & 1) as i32,
                side,
            }
        });
        Self {
            x,
            y,
            z,
            palette: Palette::for_biome(biome),
            houses,
        }
    }

    fn block(&self, x: i32, y: i32, z: i32) -> Option<Block> {
        let x = x.checked_sub(self.x)?;
        let y = y.checked_sub(self.y)?;
        let z = z.checked_sub(self.z)?;
        if !(-FOOTPRINT_RADIUS..=FOOTPRINT_RADIUS).contains(&x)
            || !(-FOOTPRINT_RADIUS..=FOOTPRINT_RADIUS).contains(&z)
            || !(-MAX_RELIEF..=CLEARANCE).contains(&y)
        {
            return None;
        }

        if y < 0 {
            return Some(self.palette.foundation);
        }
        for house in &self.houses {
            if let Some(block) = house.block(x, y, z, &self.palette) {
                return Some(block);
            }
        }
        if y > 0 {
            return Some(Block::Air);
        }
        Some(if x.abs() <= 3 && z.abs() <= 3 {
            Block::Stone // Central plaza; keep its center open for spawning.
        } else if z.abs() <= 1
            || self
                .houses
                .iter()
                .any(|house| x == house.x && (2..=4).contains(&(z * house.side)))
        {
            self.palette.road
        } else {
            self.palette.ground
        })
    }
}

struct House {
    x: i32,
    width: i32,
    depth: i32,
    side: i32,
}

impl House {
    fn block(&self, x: i32, y: i32, z: i32, palette: &Palette) -> Option<Block> {
        let dx = x - self.x;
        let dz = z * self.side - (5 + self.depth);
        let (ax, az) = (dx.abs(), dz.abs());
        // A one-block overhang and a stepped gable roof with a ridge along z.
        if ax <= self.width + 1 && az <= self.depth + 1 {
            let roof_y = 4 + self.width - ax.max(1);
            if y == roof_y {
                return Some(palette.roof);
            }
            if ax <= self.width && az <= self.depth && (4..roof_y).contains(&y) {
                return Some(if az == self.depth {
                    palette.wall
                } else {
                    Block::Air
                });
            }
        }
        if ax > self.width || az > self.depth || y > 3 {
            return None;
        }
        Some(match y {
            0 => palette.floor,
            1 | 2 if dx == 0 && dz == -self.depth => Block::Air,
            _ if ax == self.width && az == self.depth => Block::Log,
            2 if (ax == self.width && dz == 0) || (az == self.depth && ax == 1) => Block::Glass,
            _ if ax == self.width || az == self.depth => palette.wall,
            _ => Block::Air,
        })
    }
}

struct Palette {
    foundation: Block,
    ground: Block,
    road: Block,
    floor: Block,
    wall: Block,
    roof: Block,
}

impl Palette {
    fn for_biome(biome: Biome) -> Self {
        match biome {
            Biome::Desert => Self {
                foundation: Block::Stone,
                ground: Block::Sand,
                road: Block::Path,
                floor: Block::Stone,
                wall: Block::Sand,
                roof: Block::Planks,
            },
            Biome::SnowyPeaks => Self {
                foundation: Block::Stone,
                ground: Block::Snow,
                road: Block::Stone,
                floor: Block::Planks,
                wall: Block::Planks,
                roof: Block::Snow,
            },
            Biome::Plains | Biome::Forest => Self {
                foundation: Block::Dirt,
                ground: Block::Grass,
                road: Block::Path,
                floor: Block::Planks,
                wall: Block::Planks,
                roof: Block::Stone,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_candidates_repeat_and_stay_separated() {
        assert_ne!(candidate(42, -1, 1), candidate(43, -1, 1));
        for rz in -REGION_RADIUS..=REGION_RADIUS {
            for rx in -REGION_RADIUS..=REGION_RADIUS {
                let (x, z) = candidate(42, rx, rz);
                assert_eq!((x, z), candidate(42, rx, rz));
                assert_eq!(x.div_euclid(REGION_SIZE), rx);
                assert_eq!(z.div_euclid(REGION_SIZE), rz);
                assert!(x.abs().max(z.abs()) > 2 * FOOTPRINT_RADIUS);
                assert!(candidate(42, rx + 1, rz).0 - x > 2 * FOOTPRINT_RADIUS);
                assert!(candidate(42, rx, rz + 1).1 - z > 2 * FOOTPRINT_RADIUS);
            }
        }
    }

    #[test]
    fn platform_levels_safe_relief_and_rejects_water_and_steep_ground() {
        let base = SEA_LEVEL + 5;
        assert_eq!(platform_height(0, 0, |_, _| Some(base)), Some(base));
        assert_eq!(
            platform_height(0, 0, |x, _| Some(base + if x > 0 { 4 } else { 0 })),
            Some(base + 2)
        );
        assert_eq!(
            platform_height(0, 0, |x, _| Some(base + if x > 0 { 5 } else { 0 })),
            None
        );
        assert_eq!(platform_height(0, 0, |_, _| Some(SEA_LEVEL)), None);
        assert_eq!(
            platform_height(0, 0, |x, z| ((x, z) != (16, 16)).then_some(base)),
            None
        );
    }

    #[test]
    fn plaza_and_doors_connect_without_blocking_the_spawn() {
        let site = Site::new(0, 0, 0, 42, Biome::Plains);
        assert!(matches!(site.block(0, 0, 0), Some(Block::Stone)));
        assert!(matches!(site.block(0, 1, 0), Some(Block::Air)));
        assert!(matches!(site.block(0, -2, 0), Some(Block::Dirt)));
        assert!(matches!(site.block(0, 2, 0), Some(Block::Air)));
        assert!(site.block(17, 0, 0).is_none());
        for house in &site.houses {
            for z in 1..=4 {
                assert!(matches!(
                    site.block(house.x, 0, z * house.side),
                    Some(Block::Path)
                ));
            }
            let door_z = 5 * house.side;
            assert!(matches!(
                site.block(house.x, 0, door_z),
                Some(Block::Planks)
            ));
            for y in 1..=2 {
                assert!(matches!(site.block(house.x, y, door_z), Some(Block::Air)));
            }
            assert!(matches!(
                site.block(house.x + 1, 2, door_z),
                Some(Block::Glass)
            ));
        }
    }
}
