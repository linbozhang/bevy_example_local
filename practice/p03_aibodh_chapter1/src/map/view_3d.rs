use bevy::prelude::*;

use super::generate::{GRID_X, GRID_Y, MAP_SEED, create_generator};

// 一格对应一个 3D 世界单位；不沿用 2D 版的 32 像素尺寸。
const TILE_SIZE: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(usize)]
enum TileKind {
    Dirt,
    Grass,
    YellowGrass,
    Water,
    Tree,
    Rock,
    Plant,
    Stump,
}

impl TileKind {
    const ALL: [Self; 8] = [
        Self::Dirt,
        Self::Grass,
        Self::YellowGrass,
        Self::Water,
        Self::Tree,
        Self::Rock,
        Self::Plant,
        Self::Stump,
    ];

    // 复用 2D 模型的素材名作为分类依据，不在 3D 视图里再定义一遍 WFC 规则。
    fn from_sprite(name: &str) -> Self {
        match name {
            "dirt" => Self::Dirt,
            name if name.starts_with("green_grass") => Self::Grass,
            name if name.starts_with("yellow_grass") => Self::YellowGrass,
            name if name.starts_with("water") => Self::Water,
            name if name.starts_with("small_tree") || name.starts_with("big_tree") => Self::Tree,
            name if name.starts_with("rock_") => Self::Rock,
            name if name.starts_with("plant_") => Self::Plant,
            name if name.starts_with("tree_stump_") => Self::Stump,
            _ => panic!("No 3D tile style for {name}"),
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Dirt => Color::srgb(0.51, 0.31, 0.16),
            Self::Grass => Color::srgb(0.29, 0.66, 0.22),
            Self::YellowGrass => Color::srgb(0.89, 0.72, 0.23),
            Self::Water => Color::srgb(0.10, 0.47, 0.87),
            Self::Tree => Color::srgb(0.08, 0.32, 0.14),
            Self::Rock => Color::srgb(0.47, 0.50, 0.57),
            Self::Plant => Color::srgb(0.68, 0.37, 0.72),
            Self::Stump => Color::srgb(0.40, 0.24, 0.14),
        }
    }

    fn size(self) -> Vec3 {
        let (width, height) = match self {
            Self::Dirt => (1.0, 0.55),
            Self::Grass => (1.0, 0.14),
            Self::YellowGrass => (1.0, 0.10),
            Self::Water => (1.0, 0.06),
            Self::Tree => (0.64, 1.70),
            Self::Rock => (0.60, 0.38),
            Self::Plant => (0.26, 0.47),
            Self::Stump => (0.42, 0.30),
        };
        Vec3::new(width * TILE_SIZE, height, width * TILE_SIZE)
    }
}

fn cell_center(x: u32, y: u32) -> Vec2 {
    Vec2::new(
        x as f32 + 0.5 - GRID_X as f32 / 2.0,
        GRID_Y as f32 / 2.0 - y as f32 - 0.5,
    ) * TILE_SIZE
}

pub(super) fn setup_map_3d(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (mut generator, definitions) = create_generator(MAP_SEED);
    let (info, data) = generator
        .generate_grid()
        .expect("WFC exhausted its retry budget");
    info!(
        "Generated {} terrain cells for 3D view (seed {MAP_SEED}): {info:?}",
        data.iter().count()
    );

    let mesh = meshes.add(Cuboid::new(1.0, 1.0, 1.0));
    let palette = TileKind::ALL.map(|kind| materials.add(StandardMaterial::from(kind.color())));
    let mut column_tops = [0.0; (GRID_X * GRID_Y) as usize];
    let terrain = commands
        .spawn((
            Name::new("Generated terrain (3D)"),
            Transform::default(),
            Visibility::default(),
        ))
        .id();

    // 库按 z 层从低到高枚举。空模型不占高度；树冠等 2D 附加图片不另生成方块。
    for (index, instance) in data.iter().enumerate() {
        let Some(asset) = definitions[instance.model_index].first() else {
            continue;
        };
        let kind = TileKind::from_sprite(asset.sprite_name());
        let pos = generator.grid().pos_from_index(index);
        let center = cell_center(pos.x, pos.y);
        let size = kind.size();
        let top = &mut column_tops[(pos.y * GRID_X + pos.x) as usize];
        commands.spawn((
            Name::new(asset.sprite_name()),
            Mesh3d(mesh.clone()),
            MeshMaterial3d(palette[kind as usize].clone()),
            Transform::from_xyz(center.x, *top + size.y * 0.5, center.y).with_scale(size),
            ChildOf(terrain),
        ));
        *top += size.y;
    }

    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(17.0, 25.0, 24.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            ..default()
        },
        Transform::from_xyz(-8.0, 12.0, 6.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_visible_model_has_a_distinct_category_style() {
        let (_, definitions) = create_generator(MAP_SEED);
        let mut seen = [false; TileKind::ALL.len()];
        for asset in definitions.iter().filter_map(|assets| assets.first()) {
            seen[TileKind::from_sprite(asset.sprite_name()) as usize] = true;
        }
        assert!(seen.into_iter().all(|present| present));
        for (i, kind) in TileKind::ALL.into_iter().enumerate() {
            assert!(kind.size().cmpgt(Vec3::ZERO).all());
            assert!(
                TileKind::ALL[..i]
                    .iter()
                    .all(|other| other.color() != kind.color())
            );
        }
    }

    #[test]
    fn startup_spawns_centered_ground_and_shared_mesh() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .add_systems(Startup, setup_map_3d);
        app.update();
        let world = app.world_mut();
        let mut query = world.query::<(&Name, &Mesh3d, &Transform)>();
        let tiles: Vec<_> = query.iter(world).collect();
        assert!(tiles.iter().all(|(_, mesh, _)| mesh.0 == tiles[0].1.0));
        let ground: Vec<_> = tiles
            .iter()
            .filter(|(name, _, _)| name.as_str() == "dirt")
            .collect();
        assert_eq!(ground.len(), (GRID_X * GRID_Y) as usize);
        let sum: Vec3 = ground.iter().map(|(_, _, t)| t.translation).sum();
        assert_eq!(sum.x, 0.0);
        assert_eq!(sum.z, 0.0);
        assert!(
            ground
                .iter()
                .all(|(_, _, t)| (t.translation.y - t.scale.y * 0.5).abs() < f32::EPSILON)
        );
        assert!(
            tiles.iter().any(|(name, _, t)| name.starts_with("water")
                && t.translation.y > TileKind::Dirt.size().y)
        );
    }
}
