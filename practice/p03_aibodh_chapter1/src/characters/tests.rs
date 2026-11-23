use std::{
    thread,
    time::{Duration, Instant},
};

use bevy::{
    asset::LoadState,
    image::{CompressedImageFormats, ImageLoader},
    prelude::*,
};

use super::{CharactersPlugin, Player, Roster, RosterHandle};
use crate::map::{
    LAND_TOP,
    generate::{PLAYER_Z, TILE_SIZE},
};

const ASSETS: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets");

fn test_app(mode: CharactersPlugin) -> (App, Entity) {
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
    .init_asset::<TextureAtlasLayout>()
    .init_resource::<ButtonInput<KeyCode>>()
    .init_resource::<Time>();
    if matches!(mode, CharactersPlugin::Billboard3d) {
        app.world_mut()
            .spawn((Camera3d::default(), Transform::IDENTITY));
    }
    app.update();
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let world = app.world();
        let handle = &world.resource::<RosterHandle>().0;
        if world.resource::<Assets<Roster>>().get(handle).is_some() {
            break;
        }
        let state = world.resource::<AssetServer>().get_load_state(handle.id());
        assert!(
            !matches!(state, Some(LoadState::Failed(_))) && Instant::now() < deadline,
            "could not load roster: {state:?}"
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
        check_frame(&app, entity, 0, 2, default_texture);
        step(&mut app, 0.5, &[ArrowUp, ArrowRight]);
        let walk = app.world().get::<Player>(entity).unwrap().position;
        assert!((walk.length() - 70.0).abs() < 0.001);
        let rendered = app.world().get::<Transform>(entity).unwrap().translation;
        match mode {
            CharactersPlugin::Sprites2d => assert_eq!(rendered, walk.extend(PLAYER_Z)),
            CharactersPlugin::Billboard3d => {
                assert!((rendered.x - walk.x / TILE_SIZE).abs() < 1e-5);
                assert!((rendered.z + walk.y / TILE_SIZE).abs() < 1e-5);
                assert!(rendered.y > LAND_TOP);
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
