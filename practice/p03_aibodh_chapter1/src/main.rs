use bevy::{
    prelude::*,
    window::{MonitorSelection, WindowMode},
};

mod characters;
mod combat;
mod enemy;
mod game_state;
mod inventory;
mod magic;
mod map;
mod network;
mod overworld;
mod particles;
mod save;
mod sound;

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let sprites_2d = args.iter().any(|arg| arg == "--2d");
    let fullscreen = args.iter().any(|arg| arg == "--fullscreen");
    let voxel_seed = if let Some(index) = args.iter().position(|arg| arg == "--overworld") {
        if sprites_2d || args.iter().any(|arg| arg == "--network") {
            eprintln!(
                "--overworld is an offline 3D mode; do not combine it with --2d or --network"
            );
            std::process::exit(2);
        }
        let seed = match args.get(index + 1).filter(|arg| !arg.starts_with("--")) {
            Some(seed) => seed.parse::<u64>().unwrap_or_else(|_| {
                eprintln!("Overworld seed must be an unsigned 64-bit integer");
                std::process::exit(2);
            }),
            None => 42,
        };
        Some(seed)
    } else {
        None
    };
    let (title, background) = if voxel_seed.is_some() {
        ("Voxel Overworld — 3D", Color::srgb(0.57, 0.74, 0.90))
    } else if sprites_2d {
        ("AIBodh Chapters 8–14 — 2D", Color::BLACK)
    } else {
        ("AIBodh Chapters 8–14 — 3D", Color::srgb(0.11, 0.15, 0.21))
    };

    let network = args
        .iter()
        .position(|arg| arg == "--network")
        .map(|i| {
            args.get(i + 1)
                .filter(|path| !path.starts_with("--"))
                .ok_or_else(|| "--network requires an identity file".to_string())
                .and_then(|path| {
                    network::NetworkPlugin::connect(sprites_2d, path)
                        .map_err(|error| error.to_string())
                })
        })
        .transpose()
        .unwrap_or_else(|error| {
            eprintln!("Network startup failed: {error}");
            std::process::exit(1)
        });
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
    } else if let Some(seed) = voxel_seed {
        app.add_plugins((
            overworld::OverworldPlugin(seed),
            characters::CharactersPlugin::Billboard3d,
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
        combat::CombatPlugin,
        sound::SoundPlugin,
    ));
    if let Some(network) = network {
        app.insert_resource(map::MapSeed(network.world_seed()))
            .add_plugins(network);
    } else {
        app.add_plugins(save::SavePlugin).add_systems(
            OnExit(game_state::GameState::Loading),
            save::restore_pending.after(game_state::SpawnSet),
        );
    }
    app.run();
}
