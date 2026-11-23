use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
};

use bevy::prelude::*;

use super::{
    collision::{CollisionMap, FOOT_RADIUS},
    generate::TILE_SIZE,
};

impl CollisionMap {
    pub(crate) fn clear_segment(&self, from: Vec2, to: Vec2) -> bool {
        if !self.can_stand(from) || !self.can_stand(to) {
            return false;
        }
        let steps = (from.distance(to) / (FOOT_RADIUS * 0.5)).ceil().max(1.0) as u32;
        let mut previous = from;
        // Check each local step, not just the total height change across the segment.
        (1..=steps).all(|step| {
            let next = if step == steps {
                to
            } else {
                from.lerp(to, step as f32 / steps as f32)
            };
            let clear = self.can_stand(next) && self.step_allowed(previous, next);
            previous = next;
            clear
        })
    }

    pub(crate) fn path(&self, start: Vec2, goal: Vec2) -> VecDeque<Vec2> {
        if self.clear_segment(start, goal) {
            return VecDeque::from([goal]);
        }
        let center = |i| self.cell_center(self.cell_coords(i));
        let (Some(base), Some(goal_cell)) = (self.cell_at(start), self.cell_at(goal)) else {
            return VecDeque::new();
        };
        let base = base.as_ivec2();
        let goal_cell = goal_cell.as_ivec2();
        // Seed the search with a center that the actual collider can reach, even
        // when the actor is already standing beside an obstacle inside its tile.
        let Some(start_index) = (-2..=2)
            .flat_map(|dy| (-2..=2).map(move |dx| base + IVec2::new(dx, dy)))
            .filter_map(|p| self.cell_index(p))
            .filter(|&i| {
                start.distance_squared(center(i)) <= (TILE_SIZE * 1.5).powi(2)
                    && self.clear_segment(start, center(i))
            })
            .min_by(|&a, &b| {
                start
                    .distance_squared(center(a))
                    .total_cmp(&start.distance_squared(center(b)))
            })
        else {
            return VecDeque::new();
        };
        let heuristic = |i| {
            let d = (self.cell_coords(i).as_ivec2() - goal_cell).abs();
            10 * d.max_element() + 4 * d.min_element()
        };
        let mut costs = vec![i32::MAX; self.cell_count()];
        let mut previous = vec![None; self.cell_count()];
        let mut queue = BinaryHeap::from([Reverse((heuristic(start_index), 0, start_index))]);
        costs[start_index] = 0;
        while let Some(Reverse((_, cost, i))) = queue.pop() {
            if cost != costs[i] {
                continue;
            }
            // A blocked goal center is fine if the real player position is reachable
            // from a neighboring cell. No direct walk-through fallback on failure.
            if center(i).distance_squared(goal) <= (TILE_SIZE * 1.5).powi(2)
                && self.clear_segment(center(i), goal)
            {
                let mut route = VecDeque::from([goal]);
                let mut current = Some(i);
                while let Some(i) = current {
                    route.push_front(center(i));
                    current = previous[i];
                }
                return route;
            }
            let p = self.cell_coords(i).as_ivec2();
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let Some(next) = self.cell_index(p + IVec2::new(dx, dy)) else {
                        continue;
                    };
                    let diagonal = dx != 0 && dy != 0;
                    if diagonal
                        && ![p + IVec2::new(dx, 0), p + IVec2::new(0, dy)]
                            .into_iter()
                            .all(|side| {
                                self.cell_index(side)
                                    .is_some_and(|j| self.can_stand(center(j)))
                            })
                    {
                        continue;
                    }
                    let next_cost = cost + if diagonal { 14 } else { 10 };
                    if next_cost < costs[next] && self.clear_segment(center(i), center(next)) {
                        costs[next] = next_cost;
                        previous[next] = Some(i);
                        queue.push(Reverse((next_cost + heuristic(next), next_cost, next)));
                    }
                }
            }
        }
        VecDeque::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn voxel_segments_allow_stairs_but_not_an_intermediate_cliff() {
        let size = UVec2::new(5, 1);
        let stairs = CollisionMap::from_voxel_surface_with_size(size, |p| Some(p.x as i32));
        let start = stairs.cell_center(UVec2::ZERO);
        let goal = stairs.cell_center(UVec2::new(4, 0));
        assert!(stairs.clear_segment(start, goal));
        assert!(stairs.clear_segment(goal, start));
        assert_eq!(stairs.path(start, goal), VecDeque::from([goal]));

        let cliff = CollisionMap::from_voxel_surface_with_size(size, |p| {
            Some(if p.x == 2 { 2 } else { 0 })
        });
        assert!(!cliff.clear_segment(start, goal));
        assert!(cliff.path(start, goal).is_empty());
    }

    #[test]
    fn voxel_path_finds_a_pass_through_a_cliff_beyond_wfc_bounds() {
        let size = UVec2::new(481, 9);
        let map = CollisionMap::from_voxel_surface_with_size(size, |p| {
            Some(if p.x < 390 {
                0
            } else if p.x == 390 && p.y == 6 {
                1
            } else {
                2
            })
        });
        let start = map.cell_center(UVec2::new(387, 4));
        let goal = map.cell_center(UVec2::new(393, 4));
        assert_eq!(map.cell_at(start), Some(UVec2::new(387, 4)));
        assert_eq!(CollisionMap::tile_at(start), None);
        assert!(!map.clear_segment(start, goal));
        let path = map.path(start, goal);
        assert_eq!(path.back(), Some(&goal));
        assert!(path.contains(&map.cell_center(UVec2::new(390, 6))));
        assert!(
            std::iter::once(start)
                .chain(path.iter().copied())
                .zip(path.iter().copied())
                .all(|(a, b)| map.clear_segment(a, b))
        );
    }

    #[test]
    fn path_routes_around_blocked_center() {
        let map = CollisionMap::from_tiles([(12, 9, "rock_1"), (12, 10, "small_tree_bottom")]);
        let start = CollisionMap::center(UVec2::new(10, 9));
        let goal = CollisionMap::center(UVec2::new(14, 9));
        assert!(!map.clear_segment(start, goal));
        let path = map.path(start, goal);
        assert!(path.len() > 2);
        assert_eq!(path.back(), Some(&goal));
        assert!(
            std::iter::once(start)
                .chain(path.iter().copied())
                .zip(path.iter().copied())
                .all(|(a, b)| map.clear_segment(a, b))
        );
    }
}
