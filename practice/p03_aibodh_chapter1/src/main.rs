use bevy::prelude::*;

mod map;
mod player;

fn main() {
    let sprites_2d = std::env::args().skip(1).any(|arg| arg == "--2d");
    let (title, background) = if sprites_2d {
        ("AIBodh Chapters 1–2", Color::BLACK)
    } else {
        (
            "AIBodh Chapter 2 — 3D WFC terrain",
            Color::srgb(0.11, 0.15, 0.21),
        )
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
                    ..default()
                }),
                ..default()
            }),
    );
    if sprites_2d {
        app.add_systems(Startup, setup_camera)
            .add_plugins((map::MapPlugin::Sprites2d, player::PlayerPlugin));
    } else {
        app.add_plugins(map::MapPlugin::Blocks3d);
    }
    app.run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
