//! The same roster, input and animation clock drive both render modes.
mod camera;
pub(crate) mod collision_debug;
mod config;
mod motion;
mod view;

use crate::{
    game_state::{GameState, LoadingError},
    map::collision::CollisionMap,
};
use bevy::{asset::LoadState, prelude::*};
pub(crate) use camera::{FollowCamera, Orbit, ground_position, map_ground};
use config::RosterLoader;
pub(crate) use config::{Action, Character, Roster};
pub(crate) use motion::Player;
use motion::Velocity;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct MovementSet;

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct ViewSet;

#[derive(Clone, Copy, Resource)]
pub enum CharactersPlugin {
    Sprites2d,
    Billboard3d,
}

#[derive(Resource)]
pub(crate) struct RosterHandle(pub(crate) Handle<Roster>);

#[derive(Resource, Default)]
struct LoadedImages(Vec<Handle<Image>>);

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(*self)
            .init_asset::<Roster>()
            .init_asset_loader::<RosterLoader>()
            .init_resource::<LoadedImages>()
            .add_systems(Startup, load_roster)
            .add_systems(OnEnter(GameState::Loading), camera::reset_tracking)
            .add_systems(Update, load_images.run_if(in_state(GameState::Loading)))
            .add_systems(
                OnExit(GameState::Loading),
                (spawn_player, view::draw_player)
                    .chain()
                    .in_set(crate::game_state::SpawnSet),
            );
        match self {
            Self::Sprites2d => {
                app.add_systems(Startup, camera::spawn_camera_2d)
                    .add_systems(
                        Update,
                        (
                            update_player,
                            motion::apply_velocity.in_set(MovementSet),
                            camera::follow_player_2d,
                            view::draw_player.in_set(ViewSet),
                        )
                            .chain()
                            .run_if(in_state(GameState::Playing)),
                    );
            }
            Self::Billboard3d => {
                app.init_resource::<camera::Orbit>()
                    .add_systems(Startup, camera::spawn_camera)
                    .add_systems(
                        Update,
                        (
                            camera::read_mouse,
                            update_player,
                            motion::apply_velocity.in_set(MovementSet),
                            camera::follow_player,
                            view::draw_player.in_set(ViewSet),
                        )
                            .chain()
                            .run_if(in_state(GameState::Playing)),
                    );
            }
        }
    }
}

fn load_roster(mut commands: Commands, server: Res<AssetServer>) {
    commands.insert_resource(RosterHandle(server.load("characters/characters.ron")));
}

fn load_images(
    server: Res<AssetServer>,
    handle: Res<RosterHandle>,
    rosters: Res<Assets<Roster>>,
    mode: Res<CharactersPlugin>,
    mut images: ResMut<LoadedImages>,
    mut error: ResMut<LoadingError>,
    map: Option<Res<CollisionMap>>,
    mut next: ResMut<NextState<GameState>>,
) {
    if error.0.is_some() {
        return;
    }
    let Some(roster) = rosters.get(&handle.0) else {
        if let Some(LoadState::Failed(cause)) = server.get_load_state(handle.0.id()) {
            error.0 = Some(format!("characters.ron: {cause}"));
        }
        return;
    };
    if images.0.is_empty() {
        images.0.extend(
            roster
                .characters
                .iter()
                .map(|entry| server.load(&entry.texture_path)),
        );
        if matches!(*mode, CharactersPlugin::Sprites2d) {
            images
                .0
                .push(server.load(crate::map::generate::TILEMAP_IMAGE));
        }
    }
    for image in &images.0 {
        if let Some(LoadState::Failed(cause)) = server.get_load_state(image.id()) {
            error.0 = Some(format!(
                "{}: {cause}",
                image.path().map(ToString::to_string).unwrap_or_default()
            ));
            return;
        }
    }
    if map.is_some()
        && images
            .0
            .iter()
            .all(|image| server.is_loaded_with_dependencies(image.id()))
    {
        next.set(GameState::Playing);
    }
}

fn spawn_player(
    mut commands: Commands,
    mode: Res<CharactersPlugin>,
    map: Res<CollisionMap>,
    roster: Res<Assets<Roster>>,
    handle: Res<RosterHandle>,
) {
    let mut player = Player::new(matches!(*mode, CharactersPlugin::Billboard3d));
    player.position = map.spawn_point();
    commands.spawn((
        Name::new("Player"),
        crate::game_state::SessionEntity,
        player,
        Velocity::default(),
        crate::magic::PlayerMagic::default(),
        crate::combat::Health::new(
            roster
                .get(&handle.0)
                .map_or(100., |r| r.characters[0].max_health),
        ),
        Transform::default(),
    ));
}

fn update_player(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    orbit: Option<Res<camera::Orbit>>,
    handle: Res<RosterHandle>,
    rosters: Res<Assets<Roster>>,
    mut cue: MessageWriter<crate::sound::SoundCue>,
    mut player: Query<(
        &mut Player,
        &mut Name,
        &mut Velocity,
        &mut crate::combat::Health,
    )>,
) {
    let (Some(roster), Ok((mut player, mut name, mut velocity, mut hp))) =
        (rosters.get(&handle.0), player.single_mut())
    else {
        return;
    };
    use KeyCode::*;
    const SLOTS: [KeyCode; 6] = [Digit1, Digit2, Digit3, Digit4, Digit5, Digit6];
    if let Some(index) = SLOTS
        .iter()
        .take(roster.characters.len())
        .position(|&key| input.just_pressed(key))
        && index != player.selected
    {
        player.select(index);
    }
    let entry = &roster.characters[player.selected];
    hp.set_max(entry.max_health);
    if name.as_str() != entry.name {
        *name = Name::new(entry.name.clone());
        info!("Selected {} (max health: {})", entry.name, entry.max_health);
    }
    let direction = motion::movement_input(&input, orbit.is_some());
    let world_direction = orbit
        .as_ref()
        .map_or(direction, |orbit| orbit.rotate_move(direction));
    let jumping = player.is_jumping();
    velocity.0 = player.update(entry, &input, direction, world_direction, time.delta_secs());
    if !jumping && player.is_jumping() {
        cue.write(crate::sound::SoundCue::Jump);
    }
}

#[cfg(test)]
mod tests;
