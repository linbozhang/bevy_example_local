use bevy::prelude::*;

use super::config::{Action, Character};
use crate::map::collision::CollisionMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CharacterState {
    Idle,
    Walking,
    Running,
    Jumping,
}

impl CharacterState {
    fn action(self) -> Action {
        match self {
            Self::Idle | Self::Walking => Action::Walk,
            Self::Running => Action::Run,
            Self::Jumping => Action::Jump,
        }
    }
}

#[derive(Component, Default)]
pub(super) struct Velocity(pub Vec2);

pub(super) fn apply_velocity(
    time: Res<Time>,
    map: Res<CollisionMap>,
    mut players: Query<(&Velocity, &mut Player)>,
) {
    for (velocity, mut player) in &mut players {
        player.position = map.move_circle(player.position, velocity.0 * time.delta_secs());
    }
}

#[derive(Clone, Copy)]
enum Facing {
    Up = 0,
    Left = 1,
    Down = 2,
    Right = 3,
}

#[derive(Component)]
pub(crate) struct Player {
    pub selected: usize,
    pub position: Vec2,
    facing: Facing,
    state: CharacterState,
    elapsed: f64,
    pub lift: f32,
}

impl Player {
    pub fn new(third_person: bool) -> Self {
        Self {
            selected: 0,
            position: Vec2::ZERO,
            facing: if third_person {
                Facing::Up
            } else {
                Facing::Down
            },
            state: CharacterState::Idle,
            elapsed: 0.0,
            lift: 0.0,
        }
    }

    pub(crate) fn facing_direction(&self) -> Vec2 {
        match self.facing {
            Facing::Up => Vec2::Y,
            Facing::Down => Vec2::NEG_Y,
            Facing::Left => Vec2::NEG_X,
            Facing::Right => Vec2::X,
        }
    }

    pub fn select(&mut self, selected: usize) {
        self.selected = selected;
        self.state = CharacterState::Idle;
        self.elapsed = 0.0;
        self.lift = 0.0;
    }

    pub(super) fn update(
        &mut self,
        entry: &Character,
        input: &ButtonInput<KeyCode>,
        direction: Vec2,
        world_direction: Vec2,
        dt: f32,
    ) -> Vec2 {
        use CharacterState::*;
        use KeyCode::*;
        let moving = direction != Vec2::ZERO;

        let old_row = self.row(entry);
        if moving {
            self.facing = if direction.x.abs() > direction.y.abs() {
                if direction.x > 0.0 {
                    Facing::Right
                } else {
                    Facing::Left
                }
            } else if direction.y > 0.0 {
                Facing::Up
            } else {
                Facing::Down
            };
        }

        let grounded = if !moving {
            Idle
        } else if input.any_pressed([ShiftLeft, ShiftRight]) {
            Running
        } else {
            Walking
        };
        let clip = &entry.animations[&self.state.action()];
        let duration = f64::from(clip.frame_time) * f64::from(clip.frame_count);
        let next = if self.state == Jumping {
            if self.elapsed + f64::from(dt) >= duration {
                grounded
            } else {
                Jumping
            }
        } else if input.just_pressed(Space) {
            Jumping
        } else {
            grounded
        };
        if self.state != next || next == Idle || (next != Jumping && old_row != self.row(entry)) {
            self.elapsed = 0.0;
        } else if next == Jumping {
            self.elapsed += f64::from(dt);
        } else {
            self.elapsed = (self.elapsed + f64::from(dt)) % duration;
        }
        self.state = next;
        self.lift = if next == Jumping {
            // Re-read jump duration when starting from another clip.
            let jump = &entry.animations[&Action::Jump];
            let progress =
                (self.elapsed / (f64::from(jump.frame_time) * f64::from(jump.frame_count))) as f32;
            4.0 * progress * (1.0 - progress)
        } else {
            0.0
        };
        let speed = match next {
            Idle | Jumping => 0.0,
            Walking => entry.base_move_speed,
            Running => entry.base_move_speed * entry.run_speed_multiplier,
        };
        world_direction.normalize_or_zero() * speed
    }

    fn row(&self, entry: &Character) -> u32 {
        let clip = &entry.animations[&self.state.action()];
        clip.start_row
            + if clip.directional {
                self.facing as u32
            } else {
                0
            }
    }

    pub(super) fn frame(&self, entry: &Character) -> UVec2 {
        let clip = &entry.animations[&self.state.action()];
        let col = (self.elapsed / f64::from(clip.frame_time)) as u32;
        UVec2::new(col.min(clip.frame_count - 1), self.row(entry))
    }
}

pub(super) fn movement_input(input: &ButtonInput<KeyCode>, wasd: bool) -> Vec2 {
    use KeyCode::*;
    [
        (ArrowLeft, KeyA, Vec2::NEG_X),
        (ArrowRight, KeyD, Vec2::X),
        (ArrowUp, KeyW, Vec2::Y),
        (ArrowDown, KeyS, Vec2::NEG_Y),
    ]
    .into_iter()
    .filter_map(|(arrow, key, direction)| {
        (input.pressed(arrow) || wasd && input.pressed(key)).then_some(direction)
    })
    .sum()
}
