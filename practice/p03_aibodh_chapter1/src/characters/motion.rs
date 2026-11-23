use bevy::prelude::*;

use super::config::{Action, Character};

#[derive(Clone, Copy)]
enum Facing {
    Up = 0,
    Left = 1,
    Down = 2,
    Right = 3,
}

#[derive(Component)]
pub(super) struct Player {
    pub selected: usize,
    pub position: Vec2,
    facing: Facing,
    action: Action,
    moving: bool,
    elapsed: f64,
    pub lift: f32,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            selected: 0,
            position: Vec2::ZERO,
            facing: Facing::Down,
            action: Action::Walk,
            moving: false,
            elapsed: 0.0,
            lift: 0.0,
        }
    }
}

impl Player {
    pub fn select(&mut self, selected: usize) {
        self.selected = selected;
        self.action = Action::Walk;
        self.moving = false;
        self.elapsed = 0.0;
        self.lift = 0.0;
    }

    pub fn update(&mut self, entry: &Character, input: &ButtonInput<KeyCode>, dt: f32) {
        use KeyCode::*;
        let direction: Vec2 = [
            (ArrowLeft, Vec2::NEG_X),
            (ArrowRight, Vec2::X),
            (ArrowUp, Vec2::Y),
            (ArrowDown, Vec2::NEG_Y),
        ]
        .into_iter()
        .filter_map(|(key, direction)| input.pressed(key).then_some(direction))
        .sum();
        let moving = direction != Vec2::ZERO;
        let running = moving && input.any_pressed([ShiftLeft, ShiftRight]);
        let speed = entry.base_move_speed
            * if running {
                entry.run_speed_multiplier
            } else {
                1.0
            };
        self.position += direction.normalize_or_zero() * speed * dt;

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

        // Jump is a one-shot. Turning changes its row, never its playback progress.
        let locomotion = if running { Action::Run } else { Action::Walk };
        if self.action == Action::Jump {
            self.elapsed += f64::from(dt);
            let clip = &entry.animations[&Action::Jump];
            let duration = f64::from(clip.frame_time) * f64::from(clip.frame_count);
            if self.elapsed >= duration {
                self.action = locomotion;
                self.elapsed = 0.0;
                self.lift = 0.0;
            } else {
                let progress = (self.elapsed / duration) as f32;
                self.lift = 4.0 * progress * (1.0 - progress);
            }
        } else if input.just_pressed(Space) {
            self.action = Action::Jump;
            self.elapsed = 0.0;
        } else {
            let changed = self.action != locomotion || self.moving != moving;
            self.action = locomotion;
            let clip = &entry.animations[&self.action];
            if changed || old_row != self.row(entry) || !moving {
                self.elapsed = 0.0;
            } else {
                let duration = f64::from(clip.frame_time) * f64::from(clip.frame_count);
                self.elapsed = (self.elapsed + f64::from(dt)) % duration;
            }
        }
        self.moving = moving;
    }

    fn row(&self, entry: &Character) -> u32 {
        let clip = &entry.animations[&self.action];
        clip.start_row
            + if clip.directional {
                self.facing as u32
            } else {
                0
            }
    }

    pub fn frame(&self, entry: &Character) -> UVec2 {
        let clip = &entry.animations[&self.action];
        let col = (self.elapsed / f64::from(clip.frame_time)) as u32;
        UVec2::new(col.min(clip.frame_count - 1), self.row(entry))
    }
}
