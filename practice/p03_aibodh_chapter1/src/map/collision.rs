//! Shared ground-plane collision in 2D pixels, regardless of renderer.
use bevy::prelude::*;

use super::generate::{GRID_X, GRID_Y, TILE_SIZE};

pub(crate) const FOOT_RADIUS: f32 = 7.0;
const SIZE: UVec2 = UVec2::new(GRID_X, GRID_Y);
pub(super) const COUNT: usize = (GRID_X * GRID_Y) as usize;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Tile {
    #[default]
    Land,
    Shore,
    Water,
    Tree,
    Rock,
    Stump,
}

impl Tile {
    pub fn half_extent(self) -> Option<Vec2> {
        let fraction = match self {
            Self::Land | Self::Shore => return None,
            Self::Water => 0.5,
            Self::Tree => 0.24,
            Self::Rock => 0.32,
            Self::Stump => 0.24,
        };
        Some(Vec2::splat(fraction * TILE_SIZE))
    }
}

#[derive(Resource)]
pub(crate) struct CollisionMap {
    // Keep water surfaces separate from props so shore conversion can see the
    // original shoreline even when the adjacent land has a tree or rock.
    surface: [Tile; COUNT],
    props: [Tile; COUNT],
}

impl CollisionMap {
    pub fn from_tiles(tiles: impl IntoIterator<Item = (u32, u32, &'static str)>) -> Self {
        let mut map = Self {
            surface: [Tile::Land; COUNT],
            props: [Tile::Land; COUNT],
        };
        for (x, y, name) in tiles {
            let index = (y * GRID_X + x) as usize;
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
        let original = map.surface;
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

    pub(super) fn index(p: IVec2) -> Option<usize> {
        (p.cmpge(IVec2::ZERO).all() && p.cmplt(SIZE.as_ivec2()).all())
            .then(|| (p.y as u32 * GRID_X + p.x as u32) as usize)
    }

    pub(super) fn coords(index: usize) -> UVec2 {
        UVec2::new(index as u32 % GRID_X, index as u32 / GRID_X)
    }

    pub fn center(p: UVec2) -> Vec2 {
        (p.as_vec2() + 0.5 - SIZE.as_vec2() * 0.5) * TILE_SIZE
    }

    pub fn surface(&self, p: UVec2) -> Tile {
        self.surface[(p.y * GRID_X + p.x) as usize]
    }

    fn tile(&self, index: usize) -> Tile {
        if self.props[index] == Tile::Land {
            self.surface[index]
        } else {
            self.props[index]
        }
    }

    pub fn tiles(&self) -> impl Iterator<Item = (Vec2, Tile)> + '_ {
        (0..COUNT).map(|i| (Self::center(Self::coords(i)), self.tile(i)))
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
        let extent = SIZE.as_vec2() * TILE_SIZE * 0.5;
        if (center.abs() + radius).cmpgt(extent).any() {
            return false;
        }
        let lo = ((center - radius + extent) / TILE_SIZE).floor().as_ivec2();
        let hi = ((center + radius + extent) / TILE_SIZE).floor().as_ivec2();
        for y in lo.y..=hi.y {
            for x in lo.x..=hi.x {
                if let Some(i) = Self::index(IVec2::new(x, y))
                    && let Some(half) = self.tile(i).half_extent()
                {
                    let nearest = (center - Self::center(Self::coords(i))).abs() - half;
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
        let clear = |p: Vec2| {
            self.can_stand(p)
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
            if clear(position + step) {
                position += step;
            } else {
                let x = position + Vec2::new(step.x, 0.0);
                if clear(x) {
                    position = x;
                }
                let y = position + Vec2::new(0.0, step.y);
                if clear(y) {
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
    use crate::map::generate::{MAP_SEED, collision_from_models, create_generator};

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
        let centered_lake = (11..=13).flat_map(|x| (7..=10).map(move |y| (x, y, "water")));
        let map = CollisionMap::from_tiles(centered_lake);
        assert!(!map.can_stand(Vec2::ZERO));
        assert!(map.can_stand(map.spawn_point()));
    }

    #[test]
    fn long_movement_cannot_tunnel_and_diagonal_movement_slides() {
        let map = CollisionMap::from_tiles([(12, 9, "rock_1")]);
        let rock = CollisionMap::center(UVec2::new(12, 9));
        let start = rock - Vec2::X * 32.0;
        let stopped = map.move_circle(start, Vec2::X * 100.0);
        assert!(stopped.x > start.x && stopped.x < rock.x);
        assert!(map.can_stand(stopped));
        let slid = map.move_circle(stopped, Vec2::new(6.0, 12.0));
        assert!(slid.y > stopped.y && map.can_stand(slid));
        let edge = map.move_circle(start, Vec2::NEG_X * 1000.0);
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
    fn default_map_exercises_deep_water_shore_and_props() {
        let (mut generator, definitions) = create_generator(MAP_SEED);
        let (_, data) = generator.generate_grid().unwrap();
        let map = collision_from_models(&generator, &definitions, data.iter());
        for expected in [Tile::Shore, Tile::Water, Tile::Tree, Tile::Rock] {
            assert!(
                map.tiles().any(|(_, t)| t == expected),
                "missing {expected:?}"
            );
        }
        assert!(map.can_stand(map.spawn_point()));
    }
}
