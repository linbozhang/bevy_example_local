use bevy::{
    prelude::*,
    window::{MonitorSelection, WindowMode},
};

mod characters;
mod enemy;
mod game_state;
mod inventory;
mod magic;
mod map;
mod particles;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let sprites_2d = args.iter().any(|arg| arg == "--2d");
    let fullscreen = args.iter().any(|arg| arg == "--fullscreen");
    let (title, background) = if sprites_2d {
        ("AIBodh Chapter 7 — 2D", Color::BLACK)
    } else {
        ("AIBodh Chapter 7 — 3D", Color::srgb(0.11, 0.15, 0.21))
    };

    let mut app = App::new();
    app.insert_resource(ClearColor(background)).add_plugins(
        DefaultPlugins
            .set(AssetPlugin {
                // 以 crate 而非当前工作目录定位资源，支持从 workspace 根目录运行。
                file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets").into(),
                ..default()
            })
            .set(ImagePlugin::default_nearest())
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: title.into(),
                    resolution: (1920, 1080).into(),
                    mode: if fullscreen {
                        WindowMode::BorderlessFullscreen(MonitorSelection::Current)
                    } else {
                        WindowMode::Windowed
                    },
                    ..default()
                }),
                ..default()
            }),
    );
    app.add_plugins(game_state::GameStatePlugin);
    if sprites_2d {
        app.add_plugins((
            map::MapPlugin::Sprites2d,
            characters::CharactersPlugin::Sprites2d,
        ));
    } else {
        app.add_plugins((
            map::MapPlugin::Blocks3d,
            characters::CharactersPlugin::Billboard3d,
        ));
    }
    app.add_plugins((
        characters::collision_debug::CollisionDebugPlugin,
        inventory::InventoryPlugin,
        magic::MagicPlugin,
        particles::ParticlePlugin,
        enemy::EnemyPlugin,
    ));
    app.run();
}
