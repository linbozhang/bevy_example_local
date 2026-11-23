use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

use bevy::{
    image::{CompressedImageFormats, ImageLoader},
    state::app::StatesPlugin,
};

use super::*;
use crate::inventory::{ItemKind, Pickable};

fn map_app(mode: MapPlugin, seed: u64) -> App {
    let mut app = App::new();
    app.add_plugins((
        MinimalPlugins,
        StatesPlugin,
        AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets").into(),
            ..default()
        },
    ))
    .init_state::<GameState>()
    .init_resource::<LoadingError>()
    .insert_resource(MapSeed(seed))
    .add_plugins(mode)
    .init_asset::<Image>()
    .register_asset_loader(ImageLoader::new(CompressedImageFormats::empty()))
    .init_asset::<Mesh>()
    .init_asset::<StandardMaterial>()
    .init_asset::<TextureAtlasLayout>();
    app
}

fn wait_for_map(app: &mut App, seed: u64) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !app.world().contains_resource::<MapReady>() {
        assert!(Instant::now() < deadline, "terrain build timed out");
        assert!(
            app.world().resource::<LoadingError>().0.is_none(),
            "{:?}",
            app.world().resource::<LoadingError>().0
        );
        assert!(!app.world().contains_resource::<CollisionMap>());
        app.update();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(app.world().resource::<MapReady>().seed, seed);
    assert!(app.world().contains_resource::<CollisionMap>());
}

fn pickups(app: &mut App, mode_2d: bool) -> BTreeMap<(u32, u32), ItemKind> {
    let world = app.world_mut();
    let mut query = world.query::<(&MapTile, &Pickable, &Transform, &Children)>();
    let mut result = BTreeMap::new();
    for (&MapTile(tile), item, root, children) in query.iter(world) {
        assert_eq!(CollisionMap::tile_at(item.position), Some(tile));
        assert_eq!(CollisionMap::center(tile), item.position);
        assert!(world.resource::<CollisionMap>().can_stand(item.position));
        assert!(!children.is_empty());
        if mode_2d {
            let sprite = world.get::<Transform>(children[0]).unwrap();
            assert_eq!(sprite.translation.truncate(), item.position);
            assert_eq!(sprite.scale, Vec3::new(2.0, 2.0, 1.0));
        } else {
            let ground = root.translation.xz() * generate::TILE_SIZE;
            assert_eq!(Vec2::new(ground.x, -ground.y), item.position);
        }
        assert!(
            result.insert((tile.x, tile.y), item.kind).is_none(),
            "duplicated pickup"
        );
    }
    assert!(!result.is_empty());
    result
}

#[test]
fn runtime_seed_yields_the_same_pickups_in_both_modes_and_can_rebuild() {
    let seed = 123;
    let mut app = map_app(MapPlugin::Sprites2d, seed);
    wait_for_map(&mut app, seed);
    let first = pickups(&mut app, true);
    app.world_mut()
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Playing);
    app.update();
    let world = app.world_mut();
    let old_root = world
        .query_filtered::<Entity, With<TerrainRoot>>()
        .single(world)
        .unwrap();
    let old_pickup = world
        .query_filtered::<Entity, With<Pickable>>()
        .iter(world)
        .next()
        .unwrap();
    world.insert_resource(MapSeed(MAP_SEED));
    world
        .resource_mut::<NextState<GameState>>()
        .set(GameState::Loading);
    app.update();
    assert!(app.world().get_entity(old_root).is_err());
    assert!(app.world().get_entity(old_pickup).is_err());
    wait_for_map(&mut app, MAP_SEED);
    assert_ne!(first, pickups(&mut app, true));
    drop(app);

    let mut app = map_app(MapPlugin::Blocks3d, seed);
    wait_for_map(&mut app, seed);
    assert_eq!(first, pickups(&mut app, false));
}
