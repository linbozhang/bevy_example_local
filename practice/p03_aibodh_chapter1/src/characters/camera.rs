use std::f32::consts::{PI, TAU};

use bevy::{
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use super::motion::Player;
use crate::map::{CameraObstacle, LAND_TOP, generate::TILE_SIZE};

const ARM_LENGTH: f32 = 4.8;
const EYE_HEIGHT: f32 = 0.9;
const MIN_ARM: f32 = 0.25;
const CLEARANCE: f32 = 0.16;
const MOUSE_SENSITIVITY: f32 = 0.003;

#[derive(Component)]
pub(super) struct FollowCamera;

#[derive(Resource)]
pub(super) struct Orbit {
    yaw: f32,
    pitch: f32,
}

impl Default for Orbit {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.28,
        }
    }
}

impl Orbit {
    fn look(&mut self, delta: Vec2) {
        self.yaw = (self.yaw - delta.x * MOUSE_SENSITIVITY).rem_euclid(TAU);
        self.pitch = (self.pitch + delta.y * MOUSE_SENSITIVITY).clamp(-PI * 0.44, PI * 0.44);
    }

    pub fn rotate_move(&self, direction: Vec2) -> Vec2 {
        // Map XY uses +Y for forward; yaw around world +Y rotates -Z toward -X.
        Vec2::from_angle(self.yaw).rotate(direction)
    }

    fn rotation(&self) -> Quat {
        Quat::from_rotation_y(self.yaw) * Quat::from_rotation_x(-self.pitch)
    }
}

pub(super) fn ground_position(position: Vec2) -> Vec3 {
    Vec3::new(position.x / TILE_SIZE, LAND_TOP, -position.y / TILE_SIZE)
}

pub(super) fn spawn_camera(mut commands: Commands, orbit: Res<Orbit>) {
    let rotation = orbit.rotation();
    commands.spawn((
        Name::new("Third-person camera"),
        FollowCamera,
        Camera3d::default(),
        Projection::Perspective(PerspectiveProjection {
            fov: PI / 3.0,
            near: 0.08,
            ..default()
        }),
        Transform::from_translation(
            ground_position(Vec2::ZERO) + Vec3::Y * EYE_HEIGHT + rotation * Vec3::Z * ARM_LENGTH,
        )
        .with_rotation(rotation),
    ));
}

pub(super) fn read_mouse(
    mut orbit: ResMut<Orbit>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    buttons: Res<ButtonInput<MouseButton>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut window: Query<(&Window, &mut CursorOptions), With<PrimaryWindow>>,
) {
    let Ok((window, mut cursor)) = window.single_mut() else {
        return;
    };
    if !window.focused || keys.just_pressed(KeyCode::Escape) {
        if cursor.grab_mode != CursorGrabMode::None {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
    } else if cursor.grab_mode == CursorGrabMode::None {
        if buttons.just_pressed(MouseButton::Left) {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
    } else {
        orbit.look(mouse_motion.delta);
    }
}

pub(super) fn follow_player(
    orbit: Res<Orbit>,
    player: Query<&Player>,
    obstacles: Query<&Transform, With<CameraObstacle>>,
    mut camera: Query<&mut Transform, (With<FollowCamera>, Without<CameraObstacle>)>,
) {
    let (Ok(player), Ok(mut camera)) = (player.single(), camera.single_mut()) else {
        return;
    };
    // Follow the player's ground track rather than bouncing with the jump animation.
    let pivot = ground_position(player.position) + Vec3::Y * EYE_HEIGHT;
    let rotation = orbit.rotation();
    let back = rotation * Vec3::Z;
    let arm = obstacles.iter().fold(ARM_LENGTH, |arm, block| {
        let half_size = block.scale * 0.5 + Vec3::splat(CLEARANCE);
        match box_entry(pivot - block.translation, back, half_size) {
            Some(distance) => arm.min(distance.max(MIN_ARM)),
            None => arm,
        }
    });
    camera.translation = pivot + back * arm;
    camera.rotation = rotation;
}

// Ray vs. an axis-aligned box, in box-local coordinates. Ignore boxes containing
// the pivot: player movement itself still doesn't collide with terrain.
fn box_entry(origin: Vec3, direction: Vec3, half: Vec3) -> Option<f32> {
    let (mut enter, mut exit) = (f32::NEG_INFINITY, f32::INFINITY);
    for axis in 0..3 {
        if direction[axis].abs() < 1e-6 {
            if origin[axis].abs() > half[axis] {
                return None;
            }
        } else {
            let t1 = (-half[axis] - origin[axis]) / direction[axis];
            let t2 = (half[axis] - origin[axis]) / direction[axis];
            enter = enter.max(t1.min(t2));
            exit = exit.min(t1.max(t2));
        }
    }
    (enter >= 0.0 && enter <= exit).then_some(enter)
}
