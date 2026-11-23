//! Shared ground-plane collision in 2D pixels, regardless of renderer.
use bevy::prelude::*;

use super::{
    LAND_TOP,
    generate::{COUNT, SIZE, TILE_SIZE},
};

pub(crate) const FOOT_RADIUS: f32 = 7.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tile {
    #[default]
    Land,
    Shore,
    Water,
    Tree,
    Rock,
    Stump,
    Wall,
}

impl Tile {
    pub fn half_extent(self) -> Option<Vec2> {
        let fraction = match self {
            Self::Land | Self::Shore => return None,
            Self::Water | Self::Wall => 0.5,
            Self::Tree => 0.24,
            Self::Rock => 0.32,
            Self::Stump => 0.24,
        };
        Some(Vec2::splat(fraction * TILE_SIZE))
    }
}

#[derive(Resource)]
pub(crate) struct CollisionMap {
    size: UVec2,
    // Keep water surfaces separate from props so shore conversion can see the
    // original shoreline even when the adjacent land has a tree or rock.
    surface: Vec<Tile>,
    props: Vec<Tile>,
    voxel_floors: Option<Vec<Option<i32>>>,
}

impl CollisionMap {
    pub fn from_tiles(tiles: impl IntoIterator<Item = (u32, u32, &'static str)>) -> Self {
        let mut map = Self {
            size: SIZE,
            surface: vec![Tile::Land; COUNT],
            props: vec![Tile::Land; COUNT],
            voxel_floors: None,
        };
        for (x, y, name) in tiles {
            let index = (y * SIZE.x + x) as usize;
            if name.starts_with("water") {
                map.surface[index] = Tile::Water;
            } else if name.starts_with("small_tree") || name.starts_with("big_tree") {
                map.props[index] = Tile::Tree;
            } else if name.starts_with("rock_") {
                map.props[index] = Tile::Rock;
            } else if name.starts_with("tree_stump_") {
                map.props[index] = Tile::Stump;
            }
        }
        let original = map.surface.clone();
        for (index, tile) in map.surface.iter_mut().enumerate() {
            if *tile != Tile::Water {
                continue;
            }
            let p = Self::coords(index).as_ivec2();
            if (-1..=1).any(|dy| {
                (-1..=1).any(|dx| {
                    Self::index(p + IVec2::new(dx, dy)).is_some_and(|i| original[i] == Tile::Land)
                })
            }) {
                *tile = Tile::Shore;
            }
        }
        map
    }

    /// Rasterize a voxel surface using the default map dimensions.
    #[cfg(test)]
    pub fn from_voxel_surface(sample: impl FnMut(UVec2) -> Option<i32>) -> Self {
        Self::from_voxel_surface_with_size(SIZE, sample)
    }

    /// Sample each cell once in row-major order, starting at (0, 0).
    /// `Some(y)` is a walkable foot elevation in world units, `None` a solid cell.
    /// The grid is centered on logical zero, so for odd dimensions `size / 2`
    /// is the cell at zero. This does not apply any 3D coordinate projection.
    pub fn from_voxel_surface_with_size(
        size: UVec2,
        mut sample: impl FnMut(UVec2) -> Option<i32>,
    ) -> Self {
        assert!(size.min_element() > 0 && size.max_element() <= i32::MAX as u32);
        let count = size.x as usize * size.y as usize;
        let voxel_floors: Vec<_> = (0..count)
            .map(|i| sample(Self::coords_for(size.x, i)))
            .collect();
        let surface = voxel_floors
            .iter()
            .map(|floor| {
                if floor.is_some() {
                    Tile::Land
                } else {
                    Tile::Wall
                }
            })
            .collect();
        Self {
            size,
            surface,
            props: vec![Tile::Land; count],
            voxel_floors: Some(voxel_floors),
        }
    }

    pub(crate) fn is_voxel(&self) -> bool {
        self.voxel_floors.is_some()
    }

    /// Foot elevation at the center's cell; WFC and missing floors use LAND_TOP.
    pub(crate) fn floor_y(&self, center: Vec2) -> f32 {
        self.voxel_floor(center).map_or(LAND_TOP, |y| y as f32)
    }

    /// Whether the floor heights permit a local step. Check footprint separately.
    pub(crate) fn step_allowed(&self, from: Vec2, to: Vec2) -> bool {
        !self.is_voxel()
            || matches!((self.voxel_floor(from), self.voxel_floor(to)),
                (Some(a), Some(b)) if a.abs_diff(b) <= 1)
    }

    fn voxel_floor(&self, position: Vec2) -> Option<i32> {
        let floors = self.voxel_floors.as_ref()?;
        let cell = self.cell_at(position)?;
        floors[Self::offset(self.size.x, cell)]
    }

    fn offset(width: u32, p: UVec2) -> usize {
        p.y as usize * width as usize + p.x as usize
    }

    fn index_for(size: UVec2, p: IVec2) -> Option<usize> {
        (p.cmpge(IVec2::ZERO).all() && p.cmplt(size.as_ivec2()).all())
            .then(|| Self::offset(size.x, p.as_uvec2()))
    }

    fn coords_for(width: u32, index: usize) -> UVec2 {
        let width = width as usize;
        UVec2::new((index % width) as u32, (index / width) as u32)
    }

    fn center_for(size: UVec2, p: UVec2) -> Vec2 {
        (p.as_vec2() + 0.5 - size.as_vec2() * 0.5) * TILE_SIZE
    }

    fn tile_at_for(size: UVec2, position: Vec2) -> Option<UVec2> {
        if !position.is_finite() {
            return None;
        }
        let cell = (position / TILE_SIZE + size.as_vec2() * 0.5)
            .floor()
            .as_ivec2();
        Self::index_for(size, cell).map(|_| cell.as_uvec2())
    }

    pub(super) fn index(p: IVec2) -> Option<usize> {
        Self::index_for(SIZE, p)
    }

    pub(super) fn coords(index: usize) -> UVec2 {
        Self::coords_for(SIZE.x, index)
    }

    pub fn center(p: UVec2) -> Vec2 {
        Self::center_for(SIZE, p)
    }

    /// Tile containing a logical ground position on the default-size WFC grid.
    #[cfg(test)]
    pub fn tile_at(position: Vec2) -> Option<UVec2> {
        Self::tile_at_for(SIZE, position)
    }

    /// Center of a cell on this map's grid, in logical pixels.
    pub(crate) fn cell_center(&self, p: UVec2) -> Vec2 {
        Self::center_for(self.size, p)
    }

    /// Cell containing a logical ground position on this map; outside is None.
    pub(crate) fn cell_at(&self, position: Vec2) -> Option<UVec2> {
        Self::tile_at_for(self.size, position)
    }

    pub(super) fn cell_index(&self, p: IVec2) -> Option<usize> {
        Self::index_for(self.size, p)
    }

    pub(super) fn cell_coords(&self, index: usize) -> UVec2 {
        Self::coords_for(self.size.x, index)
    }

    pub(super) fn cell_count(&self) -> usize {
        self.surface.len()
    }

    pub fn surface(&self, p: UVec2) -> Tile {
        self.surface[Self::offset(self.size.x, p)]
    }

    fn tile(&self, index: usize) -> Tile {
        if self.props[index] == Tile::Land {
            self.surface[index]
        } else {
            self.props[index]
        }
    }

    pub fn tiles(&self) -> impl Iterator<Item = (Vec2, Tile)> + '_ {
        (0..self.cell_count()).map(|i| (self.cell_center(self.cell_coords(i)), self.tile(i)))
    }

    pub fn spawn_point(&self) -> Vec2 {
        if self.can_stand(Vec2::ZERO) {
            return Vec2::ZERO;
        }
        self.tiles()
            .filter(|(p, tile)| *tile == Tile::Land && self.can_stand(*p))
            .min_by(|(a, _), (b, _)| a.length_squared().total_cmp(&b.length_squared()))
            .expect("generated map must have a walkable land tile")
            .0
    }

    pub fn can_stand(&self, center: Vec2) -> bool {
        let radius = Vec2::splat(FOOT_RADIUS);
        let extent = self.size.as_vec2() * TILE_SIZE * 0.5;
        if !center.is_finite() || (center.abs() + radius).cmpgt(extent).any() {
            return false;
        }
        let lo = ((center - radius + extent) / TILE_SIZE).floor().as_ivec2();
        let hi = ((center + radius + extent) / TILE_SIZE).floor().as_ivec2();
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                if let Some(i) = self.cell_index(IVec2::new(x, y))
                    && let Some(half) = self.tile(i).half_extent()
                {
                    let nearest = (center - self.cell_center(self.cell_coords(i))).abs() - half;
                    if nearest.max(Vec2::ZERO).length_squared() < FOOT_RADIUS * FOOT_RADIUS {
                        return false;
                    }
                }
            }
        }
        true
    }

    #[cfg(test)]
    pub fn move_circle(&self, position: Vec2, delta: Vec2) -> Vec2 {
        self.move_circle_avoiding(position, delta, [])
    }

    pub fn move_circle_avoiding(
        &self,
        mut position: Vec2,
        delta: Vec2,
        others: impl IntoIterator<Item = Vec2> + Clone,
    ) -> Vec2 {
        let clear = |from: Vec2, p: Vec2| {
            self.can_stand(p)
                && self.step_allowed(from, p)
                && others
                    .clone()
                    .into_iter()
                    .all(|other| p.distance_squared(other) >= (2.0 * FOOT_RADIUS).powi(2))
        };
        if delta == Vec2::ZERO {
            return position;
        }
        // Sweep in small increments (shorter than even the smallest obstacle).
        // If diagonal movement is blocked, try its axes to slide along walls.
        let steps = (delta.length() / (FOOT_RADIUS * 0.5)).ceil() as u32;
        let step = delta / steps.max(1) as f32;
        for _ in 0..steps {
            let previous = position;
            if clear(position, position + step) {
                position += step;
            } else {
                let x = position + Vec2::new(step.x, 0.0);
                if clear(position, x) {
                    position = x;
                }
                let y = position + Vec2::new(0.0, step.y);
                if clear(position, y) {
                    position = y;
                }
            }
            if previous == position {
                break;
            }
        }
        position
    }
}

impl Default for CollisionMap {
    fn default() -> Self {
        Self::from_tiles([])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::generate::{MAP_SEED, build_terrain};

    #[test]
    fn shoreline_and_tree_canopy_are_passable_but_deep_water_and_bases_are_not() {
        let lake = (6..9).flat_map(|y| (6..9).map(move |x| (x, y, "water")));
        let map =
            CollisionMap::from_tiles(lake.chain([(12, 9, "small_tree_bottom"), (13, 9, "rock_1")]));
        assert_eq!(map.surface(UVec2::new(6, 6)), Tile::Shore);
        assert_eq!(map.surface(UVec2::new(7, 7)), Tile::Water);
        for (x, y, passable) in [
            (6, 6, true),
            (7, 7, false),
            (12, 9, false),
            (12, 10, true),
            (13, 9, false),
        ] {
            assert_eq!(
                map.can_stand(CollisionMap::center(UVec2::new(x, y))),
                passable
            );
        }
        assert!(map.can_stand(map.spawn_point()));
        let mid = SIZE / 2;
        let centered_lake = (mid.x - 1..=mid.x + 1)
            .flat_map(|x| (mid.y - 1..=mid.y + 1).map(move |y| (x, y, "water")));
        let map = CollisionMap::from_tiles(centered_lake);
        assert!(!map.can_stand(Vec2::ZERO));
        assert!(map.can_stand(map.spawn_point()));
    }

    #[test]
    fn long_movement_cannot_tunnel_and_diagonal_movement_slides() {
        let map = CollisionMap::from_tiles([(12, 9, "rock_1")]);
        let rock = CollisionMap::center(UVec2::new(12, 9));
        let start = rock - Vec2::X * TILE_SIZE;
        let stopped = map.move_circle(start, Vec2::X * 100.0);
        assert!(stopped.x > start.x && stopped.x < rock.x);
        assert!(map.can_stand(stopped));
        let slid = map.move_circle(stopped, Vec2::new(6.0, 12.0));
        assert!(slid.y > stopped.y && map.can_stand(slid));
        let edge = map.move_circle(
            start,
            Vec2::NEG_X * (3.0 * TILE_SIZE + start.x - CollisionMap::center(UVec2::ZERO).x),
        );
        assert!(map.can_stand(edge) && !map.can_stand(edge - Vec2::X * 4.0));
    }

    #[test]
    fn moving_actors_do_not_tunnel_through_stationary_actors() {
        let map = CollisionMap::default();
        let p = map.move_circle_avoiding(Vec2::NEG_X * 30.0, Vec2::X * 100.0, [Vec2::ZERO]);
        assert!(p.x > -30.0 && p.x <= -FOOT_RADIUS * 2.0);
        let slid = map.move_circle_avoiding(p, Vec2::new(5.0, 10.0), [Vec2::ZERO]);
        assert!(slid.y > p.y && slid.length() >= FOOT_RADIUS * 2.0);
    }

    #[test]
    fn voxel_surface_uses_full_wall_tiles_and_world_foot_elevation() {
        let wall = UVec2::new(12, 9);
        let mut sampled = 0;
        let map = CollisionMap::from_voxel_surface(|cell| {
            assert_eq!(cell, CollisionMap::coords(sampled));
            sampled += 1;
            (cell != wall).then_some(-4)
        });
        assert!(map.is_voxel());
        assert_eq!(sampled, COUNT);
        let center = CollisionMap::center(wall);
        let neighbor = center - Vec2::X * TILE_SIZE;
        assert_eq!(map.floor_y(neighbor), -4.0);
        assert_eq!(map.surface(wall), Tile::Wall);
        assert!(map.tiles().any(|tile| tile == (center, Tile::Wall)));
        assert!(map.tiles().any(|tile| tile == (neighbor, Tile::Land)));
        let edge = center - Vec2::X * TILE_SIZE * 0.5;
        assert!(!map.can_stand(edge - Vec2::X * (FOOT_RADIUS - 1.0)));
        assert!(map.can_stand(edge - Vec2::X * (FOOT_RADIUS + 1.0)));
        assert!(!map.step_allowed(neighbor, center));
        assert_eq!(map.spawn_point(), Vec2::ZERO);
    }

    #[test]
    fn voxel_movement_climbs_one_block_and_slides_along_a_two_block_cliff() {
        let map = CollisionMap::from_voxel_surface_with_size(UVec2::new(5, 5), |p| {
            Some(match p.x {
                0 | 1 => 0,
                2 => 1,
                _ => 3,
            })
        });
        let low = -Vec2::X * TILE_SIZE;
        let mid = Vec2::ZERO;
        let high = Vec2::X * TILE_SIZE;
        assert!(map.step_allowed(low, mid) && map.step_allowed(mid, low));
        assert!(!map.step_allowed(mid, high) && !map.step_allowed(high, mid));
        assert!(map.move_circle(low, mid - low).distance(mid) < 0.001);
        assert!(map.move_circle(mid, low - mid).distance(low) < 0.001);
        let slid = map.move_circle(mid, Vec2::splat(TILE_SIZE));
        assert!(slid.x > 0.0 && slid.x < TILE_SIZE * 0.5);
        assert!((slid.y - TILE_SIZE).abs() < 0.001);
        assert_eq!(map.floor_y(slid), 1.0);
        assert_eq!(map.floor_y(map.move_circle(high, mid - high)), 3.0);

        let wfc = CollisionMap::default();
        assert!(!wfc.is_voxel());
        assert_eq!(wfc.floor_y(mid), LAND_TOP);
        assert!(wfc.step_allowed(mid, high));
    }

    #[test]
    fn default_map_exercises_deep_water_shore_and_props() {
        let map = build_terrain(MAP_SEED).unwrap().collision;
        for expected in [Tile::Shore, Tile::Water, Tile::Tree, Tile::Rock] {
            assert!(
                map.tiles().any(|(_, t)| t == expected),
                "missing {expected:?}"
            );
        }
        assert!(map.can_stand(map.spawn_point()));
    }
}
