use bevy::prelude::*;

mod player;

fn main() {
    App::new()
        .insert_resource(ClearColor(Color::WHITE))
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    // 以 crate 而非当前工作目录定位资源，支持从 workspace 根目录运行。
                    file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/src/assets").into(),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "AIBodh Chapter 1 — Arrow keys to move".into(),
                        resolution: (960, 640).into(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, setup_camera)
        .add_plugins(player::PlayerPlugin)
        .run();
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera2d);
}
