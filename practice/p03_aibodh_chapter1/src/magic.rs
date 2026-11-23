//! Visual-only spells; no damage or projectile collision in this chapter.
use bevy::prelude::*;

use crate::{
    characters::{CharactersPlugin, Orbit, Player, ViewSet, ground_position},
    game_state::GameState,
    map::generate::{TILE_SIZE, depth_from_feet},
    particles::{Emitter, Layer, ParticleSet},
};

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(crate) enum Power {
    #[default]
    Fire,
    Arcane,
    Shadow,
    Poison,
}

impl Power {
    fn layers(self) -> (Layer, Option<Layer>) {
        let (count, lifetime, speed, spread, radius, size, spin, rise, color, core) = match self {
            Self::Fire => (
                12,
                0.85,
                290.,
                0.23,
                10.,
                28.,
                4.,
                30.,
                [1.6, 0.24, 0.035],
                Some([2.1, 0.95, 0.24]),
            ),
            Self::Arcane => (
                9,
                1.0,
                280.,
                0.08,
                5.,
                22.,
                2.,
                0.,
                [0.14, 0.52, 1.6],
                Some([0.66, 1.08, 1.9]),
            ),
            Self::Shadow => (
                16,
                0.5,
                470.,
                0.045,
                0.,
                23.,
                10.,
                0.,
                [0.58, 0.10, 1.2],
                Some([0.99, 0.35, 1.6]),
            ),
            Self::Poison => (
                16,
                1.35,
                150.,
                0.33,
                15.,
                34.,
                2.,
                22.,
                [0.22, 1.1, 0.12],
                None,
            ),
        };
        let curve = |[r, g, b]: [f32; 3], strength| {
            [
                LinearRgba::new(r, g, b, strength),
                LinearRgba::new(r * 0.62, g * 0.62, b * 0.62, strength * 0.52),
                LinearRgba::new(r * 0.22, g * 0.22, b * 0.22, 0.),
            ]
        };
        let primary = Layer {
            count,
            lifetime,
            speed,
            spread,
            radius,
            size,
            spin,
            rise,
            colors: curve(color, 0.74),
        };
        let core = core.map(|color| Layer {
            count: count / 2,
            lifetime: lifetime * 0.8,
            spread: spread * 0.45,
            radius: radius * 0.45,
            size: size * 0.58,
            colors: curve(color, 0.95),
            ..primary
        });
        (primary, core)
    }
}

#[derive(Component, Default)]
pub(crate) struct PlayerMagic {
    power: Power,
    cooldown: f32,
}

#[derive(Component)]
struct SpellLabel;

pub(crate) struct MagicPlugin;

impl Plugin for MagicPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnExit(GameState::Loading), spawn_label)
            .add_systems(
                Update,
                cast.after(ViewSet)
                    .before(ParticleSet)
                    .run_if(in_state(GameState::Playing)),
            );
    }
}

fn spawn_label(mut commands: Commands) {
    commands.spawn((
        SpellLabel,
        Text::new(label(Power::Fire)),
        TextFont {
            font_size: FontSize::Px(20.),
            ..default()
        },
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.),
            right: Val::Px(12.),
            padding: UiRect::all(Val::Px(10.)),
            ..default()
        },
        BackgroundColor(Color::srgba(0., 0., 0., 0.72)),
    ));
}

fn label(power: Power) -> String {
    format!("{power:?}  |  F5–F8  ·  Ctrl to cast")
}

fn cast(
    mut commands: Commands,
    mode: Res<CharactersPlugin>,
    time: Res<Time>,
    orbit: Option<Res<Orbit>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut player: Single<(&Player, &mut PlayerMagic)>,
    mut hud: Single<&mut Text, With<SpellLabel>>,
) {
    use KeyCode::{ControlLeft, ControlRight, F5, F6, F7, F8};
    let (player, magic) = &mut *player;
    magic.cooldown = (magic.cooldown - time.delta_secs()).max(0.0);
    if let Some((_, power)) = [
        (F5, Power::Fire),
        (F6, Power::Arcane),
        (F7, Power::Shadow),
        (F8, Power::Poison),
    ]
    .into_iter()
    .find(|(key, _)| keys.just_pressed(*key))
    {
        magic.power = power;
        hud.0 = label(power);
    }
    if magic.cooldown > 0.0 || !keys.any_just_pressed([ControlLeft, ControlRight]) {
        return;
    }
    magic.cooldown = 0.5;
    let facing = player.facing_direction();
    let direction = orbit
        .as_ref()
        .map_or(facing, |orbit| orbit.rotate_move(facing));
    spawn_spell(
        &mut commands,
        *mode,
        magic.power,
        player.position,
        direction,
        24.0 + player.lift * 20.0,
    );
}

pub(crate) fn spawn_spell(
    commands: &mut Commands,
    mode: CharactersPlugin,
    power: Power,
    position: Vec2,
    direction: Vec2,
    height: f32,
) {
    let offset = position + direction * 16.0;
    let origin = match mode {
        CharactersPlugin::Sprites2d => Vec3::new(
            offset.x,
            offset.y + height,
            depth_from_feet(offset.y) + 0.001,
        ),
        CharactersPlugin::Billboard3d => ground_position(offset) + Vec3::Y * (height / TILE_SIZE),
    };
    let (primary, core) = power.layers();
    for layer in std::iter::once(primary).chain(core) {
        commands.spawn(Emitter {
            origin,
            direction,
            layer,
            interval: None,
        });
    }
}
