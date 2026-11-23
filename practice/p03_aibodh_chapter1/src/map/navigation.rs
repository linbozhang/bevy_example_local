use std::{
    cmp::Reverse,
    collections::{BinaryHeap, VecDeque},
};

use bevy::prelude::*;

use super::{
    collision::{COUNT, CollisionMap, FOOT_RADIUS},
    generate::{GRID_X, GRID_Y, TILE_SIZE},
};

impl CollisionMap {
    pub(crate) fn clear_segment(&self, from: Vec2, to: Vec2) -> bool {
        let steps = (from.distance(to) / (FOOT_RADIUS * 0.5)).ceil().max(1.0) as u32;
        (0..=steps).all(|step| self.can_stand(from.lerp(to, step as f32 / steps as f32)))
    }

    pub(crate) fn path(&self, start: Vec2, goal: Vec2) -> VecDeque<Vec2> {
        if self.clear_segment(start, goal) {
            return VecDeque::from([goal]);
        }
        let center = |i| Self::center(Self::coords(i));
        let cell = |p: Vec2| {
            (p / TILE_SIZE + Vec2::new(GRID_X as f32, GRID_Y as f32) * 0.5)
                .floor()
                .as_ivec2()
        };
        // Seed the search with a center that the actual collider can reach, even
        // when the actor is already standing beside an obstacle inside its tile.
        let Some(start_index) = (0..COUNT)
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
        let goal_cell = cell(goal);
        let heuristic = |i| {
            let d = (Self::coords(i).as_ivec2() - goal_cell).abs();
            10 * d.max_element() + 4 * d.min_element()
        };
        let mut costs = [i32::MAX; COUNT];
        let mut previous = [None; COUNT];
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
            let p = Self::coords(i).as_ivec2();
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    if dx == 0 && dy == 0 {
                        continue;
                    }
                    let Some(next) = Self::index(p + IVec2::new(dx, dy)) else {
                        continue;
                    };
                    let diagonal = dx != 0 && dy != 0;
                    if diagonal
                        && ![p + IVec2::new(dx, 0), p + IVec2::new(0, dy)]
                            .into_iter()
                            .all(|side| {
                                Self::index(side).is_some_and(|j| self.can_stand(center(j)))
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
