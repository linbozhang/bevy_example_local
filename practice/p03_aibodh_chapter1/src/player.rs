//! Adapted from AIBodh Chapter 1; see ../LICENSE-MIT and ../README.md.
use bevy::prelude::*;

const TILE_SIZE: u32 = 64;
// 原图是 832×3456：每行实际有 13 格，不是文章代码中的 9 格。
const ATLAS_COLUMNS: usize = 13;
const ATLAS_ROWS: u32 = 54;
const WALK_FRAMES: usize = 9;
const MOVE_SPEED: f32 = 140.0;
const ANIM_DT: f32 = 0.1;

#[derive(Component)]
struct Player;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Facing {
    Up,
    Left,
    Down,
    Right,
}

#[derive(Component, Deref, DerefMut)]
struct AnimationTimer(Timer);

#[derive(Component)]
struct AnimationState {
    facing: Facing,
    moving: bool,
    was_moving: bool,
}

pub struct PlayerPlugin;

impl Plugin for PlayerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, spawn_player)
            // 共享可写组件只能防止并发，chain 才保证先移动、再选动画帧。
            .add_systems(Update, (move_player, animate_player).chain());
    }
}

fn spawn_player(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
) {
    let image = asset_server.load("player_spritesheet.png");
    let layout = layouts.add(TextureAtlasLayout::from_grid(
        UVec2::splat(TILE_SIZE),
        ATLAS_COLUMNS as u32,
        ATLAS_ROWS,
        None,
        None,
    ));
    commands.spawn((
        Name::new("Player"),
        Sprite::from_atlas_image(
            image,
            TextureAtlas {
                layout,
                index: row_start_index(Facing::Down),
            },
        ),
        Transform::from_xyz(0.0, 0.0, crate::map::generate::PLAYER_Z),
        Player,
        AnimationState {
            facing: Facing::Down,
            moving: false,
            was_moving: false,
        },
        AnimationTimer(Timer::from_seconds(ANIM_DT, TimerMode::Repeating)),
    ));
}

fn move_player(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    mut player: Query<(&mut Transform, &mut AnimationState), With<Player>>,
) {
    let Ok((mut transform, mut anim)) = player.single_mut() else {
        return;
    };
    let mut direction = Vec2::ZERO;
    for (key, delta) in [
        (KeyCode::ArrowLeft, Vec2::NEG_X),
        (KeyCode::ArrowRight, Vec2::X),
        (KeyCode::ArrowUp, Vec2::Y),
        (KeyCode::ArrowDown, Vec2::NEG_Y),
    ] {
        if input.pressed(key) {
            direction += delta;
        }
    }
    anim.moving = direction != Vec2::ZERO;
    if !anim.moving {
        return;
    }
    transform.translation += (direction.normalize() * MOVE_SPEED * time.delta_secs()).extend(0.0);
    // 与文章一致：对角线同等权重时选择竖直朝向。
    anim.facing = if direction.x.abs() > direction.y.abs() {
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

fn animate_player(
    time: Res<Time>,
    mut player: Query<(&mut AnimationState, &mut AnimationTimer, &mut Sprite), With<Player>>,
) {
    let Ok((mut anim, mut timer, mut sprite)) = player.single_mut() else {
        return;
    };
    // None 表示 Sprite 没有配置图集，不是图片尚未加载完成。
    let Some(atlas) = sprite.texture_atlas.as_mut() else {
        return;
    };
    let row_start = row_start_index(anim.facing);
    let mut col = atlas.index % ATLAS_COLUMNS;
    if atlas.index / ATLAS_COLUMNS != row_zero_based(anim.facing) || col >= WALK_FRAMES {
        atlas.index = row_start;
        col = 0;
        timer.reset();
    }
    if anim.moving {
        if !anim.was_moving {
            // 刚按下时立即前进一帧；之后按完整间隔播放。
            atlas.index = row_start + (col + 1) % WALK_FRAMES;
            timer.reset();
        } else {
            timer.tick(time.delta());
            // 一帧跨过多个间隔时也补齐，避免低帧率让动画变慢。
            let steps = timer.times_finished_this_tick() as usize;
            atlas.index = row_start + (col + steps % WALK_FRAMES) % WALK_FRAMES;
        }
    } else if anim.was_moving {
        // 与教程相同：松键后保留当前姿势，不跳回第一帧。
        timer.reset();
    }
    anim.was_moving = anim.moving;
}

fn row_start_index(facing: Facing) -> usize {
    row_zero_based(facing) * ATLAS_COLUMNS
}

fn row_zero_based(facing: Facing) -> usize {
    match facing {
        Facing::Up => 8,
        Facing::Left => 9,
        Facing::Down => 10,
        Facing::Right => 11,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn test_app() -> (App, Entity) {
        let mut app = App::new();
        app.insert_resource(Time::<()>::default())
            .init_resource::<ButtonInput<KeyCode>>()
            .add_systems(Update, (move_player, animate_player).chain());
        let entity = app
            .world_mut()
            .spawn((
                Player,
                Transform::default(),
                AnimationState {
                    facing: Facing::Down,
                    moving: false,
                    was_moving: false,
                },
                AnimationTimer(Timer::from_seconds(ANIM_DT, TimerMode::Repeating)),
                Sprite {
                    texture_atlas: Some(TextureAtlas {
                        layout: default(),
                        index: row_start_index(Facing::Down),
                    }),
                    ..default()
                },
            ))
            .id();
        (app, entity)
    }

    fn step(app: &mut App, secs: f32, keys: &[KeyCode]) {
        let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        input.reset_all();
        for key in keys {
            input.press(*key);
        }
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_secs_f32(secs));
        app.world_mut().run_schedule(Update);
    }

    fn frame(app: &App, entity: Entity) -> usize {
        app.world()
            .get::<Sprite>(entity)
            .unwrap()
            .texture_atlas
            .as_ref()
            .unwrap()
            .index
    }

    #[test]
    fn atlas_matches_bundled_png_and_walking_rows() {
        let png = include_bytes!("assets/player_spritesheet.png");
        let width = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let height = u32::from_be_bytes(png[20..24].try_into().unwrap());
        assert_eq!(width, TILE_SIZE * ATLAS_COLUMNS as u32);
        assert_eq!(height, TILE_SIZE * ATLAS_ROWS);
        for (facing, row) in [
            (Facing::Up, 8),
            (Facing::Left, 9),
            (Facing::Down, 10),
            (Facing::Right, 11),
        ] {
            assert_eq!(row_start_index(facing), row * ATLAS_COLUMNS);
            assert!(row_start_index(facing) + WALK_FRAMES <= ATLAS_COLUMNS * ATLAS_ROWS as usize);
        }
    }

    #[test]
    fn four_directions_move_and_select_animation_in_same_update() {
        for (key, facing, direction) in [
            (KeyCode::ArrowLeft, Facing::Left, Vec2::NEG_X),
            (KeyCode::ArrowRight, Facing::Right, Vec2::X),
            (KeyCode::ArrowUp, Facing::Up, Vec2::Y),
            (KeyCode::ArrowDown, Facing::Down, Vec2::NEG_Y),
        ] {
            let (mut app, entity) = test_app();
            step(&mut app, 0.5, &[key]);
            let pos = app.world().get::<Transform>(entity).unwrap().translation;
            assert!((pos.truncate() - direction * 70.0).length() < 0.001);
            assert_eq!(pos.z, 0.0);
            assert_eq!(frame(&app, entity), row_start_index(facing) + 1);
        }
    }

    #[test]
    fn diagonal_is_normalized_and_opposing_keys_cancel() {
        let (mut app, entity) = test_app();
        step(&mut app, 1.0, &[KeyCode::ArrowRight, KeyCode::ArrowUp]);
        let pos = app.world().get::<Transform>(entity).unwrap().translation;
        assert!((pos.length() - MOVE_SPEED).abs() < 0.001);
        assert_eq!(frame(&app, entity) / ATLAS_COLUMNS, 8);
        step(&mut app, 1.0, &[KeyCode::ArrowLeft, KeyCode::ArrowRight]);
        assert_eq!(
            app.world().get::<Transform>(entity).unwrap().translation,
            pos
        );
        assert!(!app.world().get::<AnimationState>(entity).unwrap().moving);
    }

    #[test]
    fn animation_wraps_catches_up_and_preserves_idle_pose() {
        let (mut app, entity) = test_app();
        step(&mut app, 0.01, &[KeyCode::ArrowDown]);
        assert_eq!(frame(&app, entity), 131);
        step(&mut app, 0.35, &[KeyCode::ArrowDown]);
        assert_eq!(frame(&app, entity), 134);
        step(&mut app, 0.5, &[KeyCode::ArrowDown]);
        assert_eq!(frame(&app, entity), 130);
        step(&mut app, 1.0, &[]);
        assert_eq!(frame(&app, entity), 130);
        step(&mut app, 0.01, &[KeyCode::ArrowRight]);
        assert_eq!(frame(&app, entity), 144);
    }

    #[test]
    fn continuous_turn_switches_row_and_resets_timer() {
        let (mut app, entity) = test_app();
        step(&mut app, 0.01, &[KeyCode::ArrowDown]);
        step(&mut app, 0.08, &[KeyCode::ArrowDown]);
        step(&mut app, 0.02, &[KeyCode::ArrowLeft]);
        assert_eq!(frame(&app, entity), 117);
        step(&mut app, 0.09, &[KeyCode::ArrowLeft]);
        assert_eq!(frame(&app, entity), 118);
    }
}
