//! Seeded fields in global coordinates; no chunk-local random state.
pub(super) const SEA_LEVEL: i32 = 26;
pub(super) const MIN_Y: i32 = -32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Block {
    Air,
    Grass,
    DryGrass,
    Dirt,
    Stone,
    Sand,
    Snow,
    Water,
    Log,
    Leaves,
    Planks,
    Path,
    Glass,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Biome {
    Plains,
    Forest,
    Desert,
    SnowyPeaks,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Column {
    pub height: i32,
    pub biome: Biome,
    pub water: Option<i32>,
    cave_opening: bool,
}

#[derive(Clone, Copy)]
pub(super) struct Terrain {
    seed: u64,
    rotation: u32,
    shift: i32,
}

pub(super) fn hash(seed: u64, x: i32, y: i32, z: i32) -> u64 {
    let mut h = seed
        ^ (x as u64).wrapping_mul(0x9e3779b185ebca87)
        ^ (y as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
        ^ (z as u64).wrapping_mul(0x165667b19e3779f9);
    h = (h ^ (h >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    h = (h ^ (h >> 27)).wrapping_mul(0x94d049bb133111eb);
    h ^ (h >> 31)
}

fn sample(seed: u64, x: i32, y: i32, z: i32) -> f32 {
    (hash(seed, x, y, z) >> 40) as f32 * (2. / 16777216.) - 1.
}

fn blend(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}
fn fade(t: f32) -> f32 {
    t * t * (3. - 2. * t)
}
fn axis(x: i32, scale: i32) -> (i32, f32) {
    (
        x.div_euclid(scale),
        fade(x.rem_euclid(scale) as f32 / scale as f32),
    )
}

fn noise2(seed: u64, x: i32, z: i32, scale: i32) -> f32 {
    let (x, u) = axis(x, scale);
    let (z, v) = axis(z, scale);
    let row = |z| blend(sample(seed, x, 0, z), sample(seed, x + 1, 0, z), u);
    blend(row(z), row(z + 1), v)
}

fn fbm(seed: u64, x: i32, z: i32, scale: i32) -> f32 {
    noise2(seed, x, z, scale) * 0.64
        + noise2(seed ^ 31, x, z, scale / 2) * 0.25
        + noise2(seed ^ 57, x, z, scale / 4) * 0.11
}

fn noise3(seed: u64, x: i32, y: i32, z: i32, scale: i32) -> f32 {
    let (x, u) = axis(x, scale);
    let (y, v) = axis(y, scale);
    let (z, w) = axis(z, scale);
    let layer = |z| {
        let row = |y| blend(sample(seed, x, y, z), sample(seed, x + 1, y, z), u);
        blend(row(y), row(y + 1), v)
    };
    blend(layer(z), layer(z + 1), w)
}

fn bump(x: f32, z: f32, cx: f32, cz: f32, rx: f32, rz: f32) -> f32 {
    let d = ((x - cx) / rx).hypot((z - cz) / rz);
    fade((1. - d).clamp(0., 1.))
}

impl Terrain {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            rotation: (hash(seed, 1, 2, 3) & 3) as u32,
            shift: (hash(seed, 2, 3, 4) % 29) as i32 - 14,
        }
    }

    pub fn column(&self, x: i32, z: i32) -> Column {
        let s = self.seed;
        let continentalness = fbm(s ^ 1, x, z, 224);
        let erosion = fbm(s ^ 2, x, z, 152);
        let weirdness = fbm(s ^ 3, x, z, 112);
        let ridge = (1. - (weirdness.abs() * 2.8).min(1.)).powi(3);
        let temperature = fbm(s ^ 4, x, z, 192);
        let humidity = fbm(s ^ 5, x, z, 144);
        let detail = fbm(s ^ 6, x, z, 32);
        let (a, b) = match self.rotation {
            0 => (x, z),
            1 => (-z, x),
            2 => (-x, -z),
            _ => (z, -x),
        };
        let a = (a + self.shift) as f32 + fbm(s ^ 7, x, z, 64) * 20.;
        let b = b as f32 + fbm(s ^ 8, x, z, 64) * 20.;
        // A seed-rotated regional prior guarantees landmarks near spawn without
        // handing each chunk an independent climate or drawing hard borders.
        let mountain = bump(a, b, 6., -151., 96., 110.);
        let basin = bump(a, b, -108., -46., 61., 52.);
        let dry = bump(a, b, 137., 7., 106., 104.);
        let wet = bump(a, b, -93., 92., 105., 89.);
        let mut height = 33. + continentalness * 13. - erosion * 5.
            + ridge * (continentalness - 0.18).max(0.) * 24.
            + detail * 5.
            + mountain * (48. + ridge * 17.)
            - basin * 24.;
        // Reserve a buildable village plateau, blending into adjacent terrain.
        let reserve = x.abs().max(z.abs()) as f32;
        let village = 1. - fade(((reserve - 21.) / 18.).clamp(0., 1.));
        height = blend(height, 33., village);
        let height = height.round().clamp(4., 88.) as i32;
        let temp = temperature + 1.65 * dry - (height - 35).max(0) as f32 * 0.023;
        let moisture = humidity + 1.55 * wet - 0.9 * dry;
        let biome = if reserve <= 21. {
            Biome::Plains
        } else if height >= 60 && temp < -0.25 {
            Biome::SnowyPeaks
        } else if temp > 0.47 && moisture < 0.0 {
            Biome::Desert
        } else if moisture > 0.17 && reserve > 22. {
            Biome::Forest
        } else {
            Biome::Plains
        };
        let water = (height < SEA_LEVEL).then_some(SEA_LEVEL);
        let cave_opening = water.is_none() && reserve > 40. && noise2(s ^ 81, x, z, 32) > 0.63;
        Column {
            height,
            biome,
            water,
            cave_opening,
        }
    }

    pub fn tree(&self, x: i32, z: i32, column: Column) -> bool {
        if column.water.is_some() || x.abs().max(z.abs()) <= 25 {
            return false;
        }
        let r = hash(self.seed ^ 37, x, 0, z) % 100;
        match column.biome {
            Biome::Forest => r < 8,
            Biome::Plains => r == 0,
            _ => false,
        }
    }

    pub fn body(&self, x: i32, y: i32, z: i32, column: Column) -> Block {
        if y < MIN_Y {
            return Block::Stone;
        }
        if y > column.height {
            return if column.water.is_some_and(|level| y <= level) {
                Block::Water
            } else {
                Block::Air
            };
        }
        let depth = column.height - y;
        // 3D noise carves linked underground voids while protecting the village
        // footing and a solid cap beneath lakes (simple deep aquifer).
        let opening = column.cave_opening;
        if y > MIN_Y + 1
            && depth >= if opening { 0 } else { 4 }
            && (column.water.is_none() || depth > 8)
            && (x.abs().max(z.abs()) > 24 || depth > 14)
        {
            let cave = noise3(self.seed ^ 19, x, y, z, 20) * 0.78
                + noise3(self.seed ^ 22, x, y, z, 9) * 0.22;
            if cave > if opening { 0.31 } else { 0.43 } {
                return if y < -20 { Block::Water } else { Block::Air };
            }
        }
        if depth == 0 {
            if column.height <= SEA_LEVEL + 1 {
                return Block::Sand;
            }
            return match column.biome {
                Biome::SnowyPeaks => Block::Snow,
                Biome::Desert => Block::Sand,
                _ => Block::Grass,
            };
        }
        if depth <= 3 {
            if column.height <= SEA_LEVEL + 1 || column.biome == Biome::Desert {
                Block::Sand
            } else {
                Block::Dirt
            }
        } else {
            Block::Stone
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nearby_landmarks_for_multiple_seeds() {
        for seed in [0, 1, 42, 999, u64::MAX] {
            let t = Terrain::new(seed);
            let mut seen = [false; 4];
            let mut wet = false;
            for x in (-240..=240).step_by(4) {
                for z in (-240..=240).step_by(4) {
                    let c = t.column(x, z);
                    seen[c.biome as usize] = true;
                    wet |= c.water.is_some();
                }
            }
            assert!(
                seen.into_iter().all(|b| b) && wet,
                "seed {seed}: {seen:?}, water={wet}"
            );
            assert!((-21..=21).all(|x| t.column(x, 20).height == 33));
        }
    }
}
