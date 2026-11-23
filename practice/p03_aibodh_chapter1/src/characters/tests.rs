use std::{
    thread,
    time::{Duration, Instant},
};

use bevy::{
    asset::LoadState,
    image::{CompressedImageFormats, ImageLoader},
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    window::{CursorGrabMode, CursorOptions, PrimaryWindow},
};

use super::{
    CharactersPlugin, Player, Roster, RosterHandle,
    camera::{FollowCamera, ground_position},
};
use crate::map::{
    CameraObstacle,
    collision::CollisionMap,
    generate::{TILE_SIZE, depth_from_feet},
};

use crate::game_state::{GameState, LoadingError};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets");

fn test_app(mode: CharactersPlugin) -> (App, Entity) {
    test_app_with(mode, |_| {})
}

fn test_app_with(mode: CharactersPlugin, configure: impl FnOnce(&mut App)) -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins((
        bevy::app::TaskPoolPlugin::default(),
        AssetPlugin {
            file_path: ASSETS.into(),
            ..default()
        },
        bevy::state::app::StatesPlugin,
        crate::game_state::GameStatePlugin,
        mode,
        crate::inventory::InventoryPlugin,
        crate::magic::MagicPlugin,
        crate::particles::ParticlePlugin,
    ))
    .init_resource::<CollisionMap>()
    .init_asset::<Image>()
    .register_asset_loader(ImageLoader::new(CompressedImageFormats::empty()))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<TextureAtlasLayout>()
    .init_resource::<AccumulatedMouseMotion>()
    .init_resource::<ButtonInput<MouseButton>>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<Time>();
    configure(&mut app);
    app.update();
    assert_eq!(
        *app.world().resource::<State<GameState>>().get(),
        GameState::Loading
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let world = app.world();
        let handle = &world.resource::<RosterHandle>().0;
        if *world.resource::<State<GameState>>().get() == GameState::Playing {
            break;
        }
        let state = world.resource::<AssetServer>().get_load_state(handle.id());
        assert!(
            !matches!(state, Some(LoadState::Failed(_))) && Instant::now() < deadline,
            "could not load game: {state:?}, {:?}",
            world.resource::<LoadingError>().0
        );
        thread::sleep(Duration::from_millis(2));
        app.update();
    }
    let entity = app
        .world_mut()
        .query_filtered::<Entity, With<Player>>()
        .single(app.world())
        .unwrap();
    (app, entity)
}

fn step(app: &mut App, dt: f32, keys: &[KeyCode]) {
    let mut input = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    input.reset_all();
    for key in keys {
        input.press(*key);
    }
    app.world_mut()
        .resource_mut::<Time>()
        .advance_by(Duration::from_secs_f32(dt));
    app.update();
}

fn check_frame(app: &App, entity: Entity, col: u32, row: u32, texture: &str) {
    let world = app.world();
    let image = if let Some(sprite) = world.get::<Sprite>(entity) {
        assert_eq!(
            sprite.texture_atlas.as_ref().unwrap().index,
            (row * 9 + col) as usize
        );
        &sprite.image
    } else {
        let handle = world
            .get::<MeshMaterial3d<StandardMaterial>>(entity)
            .unwrap();
        let material = world
            .resource::<Assets<StandardMaterial>>()
            .get(&handle.0)
            .unwrap();
        let center = material.uv_transform.transform_point2(Vec2::splat(0.5));
        let expected = (Vec2::new(col as f32, row as f32) + 0.5) / Vec2::new(9., 12.);
        assert!(center.abs_diff_eq(expected, 1e-6));
        material.base_color_texture.as_ref().unwrap()
    };
    assert_eq!(image.path().unwrap().path().to_str().unwrap(), texture);
}

#[test]
fn both_modes_share_character_selection_movement_and_animation() {
    use KeyCode::*;
    for mode in [CharactersPlugin::Sprites2d, CharactersPlugin::Billboard3d] {
        let (mut app, entity) = test_app(mode);
        let default_texture = "characters/adventurer.png";
        let idle_row = if matches!(mode, CharactersPlugin::Sprites2d) {
            2
        } else {
            0
        };
        check_frame(&app, entity, 0, idle_row, default_texture);
        step(&mut app, 0.5, &[ArrowUp, ArrowRight]);
        let walk = app.world().get::<Player>(entity).unwrap().position;
        assert!((walk.length() - 70.0).abs() < 0.001);
        let rendered = app.world().get::<Transform>(entity).unwrap().translation;
        match mode {
            CharactersPlugin::Sprites2d => {
                assert_eq!(
                    rendered,
                    (walk + Vec2::Y * (64.0 / 3.0)).extend(depth_from_feet(walk.y) + 0.0001)
                )
            }
            CharactersPlugin::Billboard3d => {
                let billboard = app.world().get::<Transform>(entity).unwrap();
                let feet = rendered - billboard.rotation * Vec3::Y * (billboard.scale.y / 3.0);
                assert!(feet.abs_diff_eq(ground_position(walk) + Vec3::Y * 0.02, 1e-5));
            }
        }
        check_frame(&app, entity, 0, 0, default_texture);
        step(&mut app, 0.5, &[ArrowUp, ShiftRight]);
        assert!(
            (app.world().get::<Player>(entity).unwrap().position.y - walk.y - 112.).abs() < 0.001
        );
        check_frame(&app, entity, 0, 4, default_texture);
        step(&mut app, 0.15, &[ArrowUp, ShiftRight]);
        check_frame(&app, entity, 2, 4, default_texture);

        step(&mut app, 0.01, &[Space]);
        check_frame(&app, entity, 0, 8, default_texture);
        step(&mut app, 0.29, &[ArrowRight]);
        check_frame(&app, entity, 3, 11, default_texture);
        assert!(app.world().get::<Player>(entity).unwrap().lift > 0.0);
        step(&mut app, 0.16, &[]);
        check_frame(&app, entity, 5, 11, default_texture);
        step(&mut app, 0.04, &[]);
        check_frame(&app, entity, 0, 3, default_texture);
        assert_eq!(app.world().get::<Player>(entity).unwrap().lift, 0.0);

        step(&mut app, 0.01, &[Space]);
        step(&mut app, 0.15, &[]);
        let before_swap = app.world().get::<Player>(entity).unwrap().position;
        step(&mut app, 0.01, &[Digit6]);
        let player = app.world().get::<Player>(entity).unwrap();
        assert_eq!(player.position, before_swap);
        assert_eq!(player.lift, 0.0);
        check_frame(&app, entity, 0, 3, "characters/starlit_oracle.png");
        step(&mut app, 1.0, &[ArrowLeft]);
        assert!(
            (app.world().get::<Player>(entity).unwrap().position.x - before_swap.x + 145.).abs()
                < 0.001
        );
    }
}

#[test]
fn roster_matches_all_six_sprite_sheets() {
    let (app, _) = test_app(CharactersPlugin::Sprites2d);
    let handle = &app.world().resource::<RosterHandle>().0;
    let roster = app
        .world()
        .resource::<Assets<Roster>>()
        .get(handle)
        .unwrap();
    assert_eq!(roster.characters.len(), 6);
    for character in &roster.characters {
        let png =
            std::fs::read(std::path::Path::new(ASSETS).join(&character.texture_path)).unwrap();
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            u32::from_be_bytes(png[16..20].try_into().unwrap()),
            character.tile_size * character.atlas_columns
        );
        assert_eq!(
            u32::from_be_bytes(png[20..24].try_into().unwrap()),
            character.tile_size * character.atlas_rows
        );
    }
}

#[test]
fn third_person_camera_follows_and_retracts_around_blocks() {
    let (mut app, player) = test_app(CharactersPlugin::Billboard3d);
    let world = app.world_mut();
    let camera = world
        .query_filtered::<Entity, With<FollowCamera>>()
        .single(world)
        .unwrap();
    let initial = *world.get::<Transform>(camera).unwrap();
    assert!(initial.translation.z > 0.0);
    step(&mut app, 0.25, &[KeyCode::KeyW]);
    let updated = *app.world().get::<Transform>(camera).unwrap();
    let traveled = app.world().get::<Player>(player).unwrap().position;
    assert!(traveled.abs_diff_eq(Vec2::Y * 35.0, 1e-5));
    let target = initial.translation + Vec3::NEG_Z * 35.0 / TILE_SIZE;
    assert!(target.z < updated.translation.z && updated.translation.z < initial.translation.z);
    step(&mut app, 2.0, &[]);
    assert!(
        app.world()
            .get::<Transform>(camera)
            .unwrap()
            .translation
            .abs_diff_eq(target, 1e-5)
    );

    let obstacle = app
        .world_mut()
        .spawn((
            CameraObstacle,
            Transform::from_translation(target + *updated.forward() * 2.0),
        ))
        .id();
    step(&mut app, 0.0, &[]);
    let shortened = *app.world().get::<Transform>(camera).unwrap();
    assert!((shortened.translation - target).dot(*updated.forward()) > 2.0);
    app.world_mut().despawn(obstacle);
    step(&mut app, 0.05, &[]);
    let recovering = app.world().get::<Transform>(camera).unwrap().translation;
    assert!(recovering.distance(target) < shortened.translation.distance(target));
    assert!(recovering.distance(target) > 0.1);
    step(&mut app, 2.0, &[]);
    step(&mut app, 0.01, &[KeyCode::Space]);
    step(&mut app, 0.15, &[]);
    assert!(app.world().get::<Player>(player).unwrap().lift > 0.0);
    assert!(
        app.world()
            .get::<Transform>(camera)
            .unwrap()
            .translation
            .abs_diff_eq(target, 1e-5)
    );
}

#[test]
fn mouse_orbit_changes_forward_movement_and_escape_releases_cursor() {
    let (mut app, player) = test_app(CharactersPlugin::Billboard3d);
    let window = app
        .world_mut()
        .spawn((
            Window {
                focused: true,
                ..default()
            },
            PrimaryWindow,
            CursorOptions::default(),
        ))
        .id();
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    step(&mut app, 0.0, &[]);
    assert_eq!(
        app.world().get::<CursorOptions>(window).unwrap().grab_mode,
        CursorGrabMode::Locked
    );
    app.world_mut()
        .resource_mut::<AccumulatedMouseMotion>()
        .delta
        .x = -std::f32::consts::FRAC_PI_2 / 0.003;
    step(&mut app, 0.25, &[KeyCode::KeyW]);
    assert!(
        app.world()
            .get::<Player>(player)
            .unwrap()
            .position
            .abs_diff_eq(Vec2::NEG_X * 35.0, 1e-5)
    );
    check_frame(&app, player, 0, 0, "characters/adventurer.png");
    step(&mut app, 0.0, &[KeyCode::Escape]);
    let cursor = app.world().get::<CursorOptions>(window).unwrap();
    assert_eq!(cursor.grab_mode, CursorGrabMode::None);
    assert!(cursor.visible);
}

#[test]
fn pause_freezes_gameplay_and_resume_does_not_spawn_another_player() {
    for mode in [CharactersPlugin::Sprites2d, CharactersPlugin::Billboard3d] {
        let (mut app, entity) = test_app(mode);
        step(&mut app, 0.1, &[KeyCode::ArrowRight]);
        let world = app.world();
        let before = *world.get::<Transform>(entity).unwrap();
        step(
            &mut app,
            1.0,
            &[KeyCode::Escape, KeyCode::ArrowRight, KeyCode::Digit6],
        );
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Paused
        );
        step(&mut app, 1.0, &[KeyCode::ArrowRight, KeyCode::Digit6]);
        assert_eq!(*app.world().get::<Transform>(entity).unwrap(), before);
        check_frame(&app, entity, 0, 3, "characters/adventurer.png");
        step(&mut app, 0.1, &[KeyCode::Escape, KeyCode::ArrowRight]);
        assert_eq!(
            *app.world().resource::<State<GameState>>().get(),
            GameState::Playing
        );
        assert!(app.world().get::<Transform>(entity).unwrap().translation.x > before.translation.x);
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<Entity, With<Player>>()
                .single(world)
                .unwrap(),
            entity
        );
    }
}

#[test]
fn both_modes_apply_collisions_to_player_velocity() {
    for mode in [CharactersPlugin::Sprites2d, CharactersPlugin::Billboard3d] {
        let (mut app, entity) = test_app(mode);
        let rock = CollisionMap::center(UVec2::new(14, 9));
        app.world_mut()
            .insert_resource(CollisionMap::from_tiles([(14, 9, "rock_1")]));
        app.world_mut().get_mut::<Player>(entity).unwrap().position = rock - Vec2::X * TILE_SIZE;
        step(&mut app, 1.0, &[KeyCode::ArrowRight, KeyCode::ShiftLeft]);
        let position = app.world().get::<Player>(entity).unwrap().position;
        assert!(position.x > rock.x - TILE_SIZE && position.x < rock.x);
        assert!(app.world().resource::<CollisionMap>().can_stand(position));
    }
}

#[test]
fn zoomed_2d_camera_settles_after_player_stops() {
    let (mut app, entity) = test_app(CharactersPlugin::Sprites2d);
    let world = app.world_mut();
    let camera = world
        .query_filtered::<Entity, With<Camera2d>>()
        .single(world)
        .unwrap();
    step(&mut app, 0.2, &[KeyCode::ArrowRight]);
    let target = app
        .world()
        .get::<Player>(entity)
        .unwrap()
        .position
        .x
        .round();
    let first = app.world().get::<Transform>(camera).unwrap().translation.x;
    assert!(0.0 < first && first < target);
    for _ in 0..120 {
        step(&mut app, 1.0 / 60.0, &[]);
    }
    assert_eq!(
        app.world().get::<Transform>(camera).unwrap().translation.x,
        target
    );
}

#[test]
fn pickups_in_both_modes_update_the_hud_and_despawn_the_whole_item() {
    use crate::inventory::{ItemKind, Pickable};
    for mode in [CharactersPlugin::Sprites2d, CharactersPlugin::Billboard3d] {
        let (mut app, _) = test_app(mode);
        let item = app
            .world_mut()
            .spawn(Pickable::new(ItemKind::Mushroom, Vec2::X * 54.0))
            .id();
        let detail = app.world_mut().spawn(ChildOf(item)).id();
        let distant = app
            .world_mut()
            .spawn(Pickable::new(ItemKind::Herb, Vec2::Y * 100.0))
            .id();
        step(&mut app, 0.0, &[]);
        assert!(app.world().get_entity(item).is_ok());
        step(&mut app, 0.2, &[KeyCode::Escape, KeyCode::ArrowRight]);
        assert!(app.world().get_entity(item).is_ok());
        step(&mut app, 0.2, &[KeyCode::Escape, KeyCode::ArrowRight]);
        assert!(app.world().get_entity(item).is_err());
        assert!(app.world().get_entity(detail).is_err());
        assert!(app.world().get_entity(distant).is_ok());
        step(&mut app, 0.0, &[KeyCode::Digit6]);
        let world = app.world_mut();
        assert!(
            world
                .query::<&Text>()
                .iter(world)
                .any(|text| text.0 == "Inventory: Mushroom: 1")
        );
    }
}

#[test]
fn generated_maps_spawn_matching_pickups() {
    use crate::{inventory::Pickable, map::MapPlugin};
    let mut maps = [MapPlugin::Sprites2d, MapPlugin::Blocks3d]
        .map(|mode| {
            let mut app = App::new();
            app.add_plugins((
                bevy::app::TaskPoolPlugin::default(),
                AssetPlugin {
                    file_path: ASSETS.into(),
                    ..default()
                },
                mode,
            ))
            .init_asset::<Image>()
            .register_asset_loader(ImageLoader::new(CompressedImageFormats::empty()))
            .init_asset::<Mesh>()
            .init_asset::<StandardMaterial>()
            .init_asset::<TextureAtlasLayout>();
            app.update();
            let world = app.world_mut();
            let mut items: Vec<_> = world
                .query::<(&Pickable, &Children)>()
                .iter(world)
                .map(|(item, children)| {
                    assert!(!children.is_empty());
                    assert!(world.resource::<CollisionMap>().can_stand(item.position));
                    (item.kind, item.position.x as i32, item.position.y as i32)
                })
                .collect();
            items.sort_unstable();
            items
        })
        .into_iter();
    let two_d = maps.next().unwrap();
    assert!(!two_d.is_empty());
    assert_eq!(two_d, maps.next().unwrap());
}

#[test]
fn spells_use_current_facing_respect_pause_and_cooldown_and_expire() {
    use crate::particles::{Emitter, Particle};
    use KeyCode::{ArrowRight, ControlLeft, ControlRight, Escape, F6, F8};
    fn count<T: Component>(app: &mut App) -> usize {
        let world = app.world_mut();
        world
            .query_filtered::<Entity, With<T>>()
            .iter(world)
            .count()
    }
    for mode in [CharactersPlugin::Sprites2d, CharactersPlugin::Billboard3d] {
        let (mut app, _) = test_app(mode);
        step(&mut app, 0.04, &[ArrowRight, F8, ControlLeft]);
        assert_eq!(count::<Particle>(&mut app), 16);
        assert_eq!(count::<Emitter>(&mut app), 0);
        let world = app.world_mut();
        assert!(
            world
                .query::<&Text>()
                .iter(world)
                .any(|text| text.0.starts_with("Poison"))
        );
        let previous: Vec<_> = world
            .query_filtered::<&Transform, With<Particle>>()
            .iter(world)
            .copied()
            .collect();
        step(&mut app, 1.0, &[Escape, ControlRight]);
        let world = app.world_mut();
        assert_eq!(
            world
                .query_filtered::<&Transform, With<Particle>>()
                .iter(world)
                .copied()
                .collect::<Vec<_>>(),
            previous
        );
        step(&mut app, 0.12, &[Escape, ControlRight]);
        assert_eq!(count::<Particle>(&mut app), 16);
        let world = app.world_mut();
        assert!(
            world
                .query_filtered::<&Transform, With<Particle>>()
                .iter(world)
                .zip(previous)
                .all(
                    |(now, old)| now.translation.x > old.translation.x && now.scale.x < old.scale.x
                )
        );
        step(&mut app, 2.0, &[]);
        assert_eq!(count::<Particle>(&mut app), 0);
        step(&mut app, 0., &[F6, ControlRight]);
        assert_eq!(count::<Particle>(&mut app), 13);
    }
}

#[test]
fn enemies_chase_cast_and_pause_in_both_modes() {
    use crate::{
        enemy::{Enemy, EnemyPlugin},
        particles::Particle,
    };
    for mode in [CharactersPlugin::Sprites2d, CharactersPlugin::Billboard3d] {
        let (mut app, player) = test_app_with(mode, |app| {
            app.add_plugins(EnemyPlugin);
        });
        let world = app.world_mut();
        let enemies: Vec<_> = world
            .query::<(Entity, &Enemy)>()
            .iter(world)
            .map(|(id, e)| (id, e.position))
            .collect();
        assert_eq!(enemies.len(), 2);
        for &(id, position) in &enemies {
            assert!(world.resource::<CollisionMap>().can_stand(position));
            match mode {
                CharactersPlugin::Sprites2d => {
                    assert!(world.get::<Sprite>(id).unwrap().texture_atlas.is_some())
                }
                CharactersPlugin::Billboard3d => {
                    assert!(world.get::<Sprite>(id).is_none());
                    assert!(
                        world
                            .get::<Children>(id)
                            .unwrap()
                            .iter()
                            .filter(|&child| world.get::<Mesh3d>(child).is_some())
                            .count()
                            >= 8
                    );
                }
            }
        }
        assert!(
            world
                .get::<Player>(player)
                .unwrap()
                .position
                .abs_diff_eq(Vec2::ZERO, 1e-5)
        );
        step(&mut app, 0.25, &[]);
        for &(id, previous) in &enemies {
            assert!(app.world().get::<Enemy>(id).unwrap().position.length() < previous.length());
        }
        for _ in 0..15 {
            step(&mut app, 0.1, &[]);
        }
        let world = app.world_mut();
        let particles = world
            .query_filtered::<Entity, With<Particle>>()
            .iter(world)
            .count();
        assert!(
            particles > 0,
            "reapers should have cast their shadow spells"
        );
        let before = world.get::<Enemy>(enemies[0].0).unwrap().position;
        step(&mut app, 0.5, &[KeyCode::Escape]);
        let world = app.world_mut();
        assert_eq!(world.get::<Enemy>(enemies[0].0).unwrap().position, before);
        assert_eq!(
            world
                .query_filtered::<Entity, With<Particle>>()
                .iter(world)
                .count(),
            particles
        );
    }
}
