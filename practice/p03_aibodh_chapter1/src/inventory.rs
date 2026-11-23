//! Chapter 5 pickups use the same ground coordinates and inventory in both views.
use bevy::prelude::*;
use std::collections::BTreeMap;

use crate::{
    characters::{MovementSet, Player},
    game_state::GameState,
};

const PICKUP_RADIUS: f32 = 40.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ItemKind {
    Herb,
    Flower,
    Mushroom,
    Fern,
}

impl ItemKind {
    fn name(self) -> &'static str {
        match self {
            Self::Herb => "Herb",
            Self::Flower => "Flower",
            Self::Mushroom => "Mushroom",
            Self::Fern => "Fern",
        }
    }
}

#[derive(Component)]
pub(crate) struct Pickable {
    pub kind: ItemKind,
    pub position: Vec2,
    pub radius: f32,
}

impl Pickable {
    pub fn new(kind: ItemKind, position: Vec2) -> Self {
        Self {
            kind,
            position,
            radius: PICKUP_RADIUS,
        }
    }
}

#[derive(Resource, Default)]
pub(crate) struct Inventory(BTreeMap<ItemKind, u32>);

impl Inventory {
    fn add(&mut self, kind: ItemKind) -> u32 {
        let count = self.0.entry(kind).or_default();
        *count += 1;
        *count
    }

    fn summary(&self) -> String {
        if self.0.is_empty() {
            return "Empty".into();
        }
        self.0
            .iter()
            .map(|(kind, count)| format!("{}: {count}", kind.name()))
            .collect::<Vec<_>>()
            .join("  |  ")
    }
}

#[derive(Component)]
struct InventoryLabel;

pub(crate) struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Inventory>()
            .add_systems(OnExit(GameState::Loading), spawn_hud)
            .add_systems(
                Update,
                (collect, update_hud)
                    .chain()
                    .after(MovementSet)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn collect(
    mut commands: Commands,
    player: Single<&Player>,
    items: Query<(Entity, &Pickable)>,
    mut bag: ResMut<Inventory>,
) {
    for (entity, item) in &items {
        if player.position.distance_squared(item.position) <= item.radius * item.radius {
            let count = bag.add(item.kind);
            info!("Picked up {} (total: {count})", item.kind.name());
            // The root owns every visual part of this pickup, but not the ground tile.
            commands.entity(entity).despawn();
        }
    }
}

fn spawn_hud(mut commands: Commands, bag: Res<Inventory>) {
    commands.spawn((
        InventoryLabel,
        Text::new(format!("Inventory: {}", bag.summary())),
        TextFont {
            font_size: FontSize::Px(20.0),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            padding: UiRect::all(Val::Px(10.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
    ));
}

fn update_hud(bag: Res<Inventory>, mut label: Single<&mut Text, With<InventoryLabel>>) {
    if bag.is_changed() {
        label.0 = format!("Inventory: {}", bag.summary());
    }
}
