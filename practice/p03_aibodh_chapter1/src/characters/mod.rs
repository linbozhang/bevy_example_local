//! The same roster, input and animation clock drive both render modes.
mod camera;
mod config;
mod motion;
mod view;

use bevy::prelude::*;
use config::{Roster, RosterLoader};
use motion::Player;

#[derive(Clone, Copy, Resource)]
pub enum CharactersPlugin {
    Sprites2d,
    Billboard3d,
}

#[derive(Resource)]
struct RosterHandle(Handle<Roster>);

impl Plugin for CharactersPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(*self)
            .init_asset::<Roster>()
            .init_asset_loader::<RosterLoader>()
            .add_systems(Startup, load_roster);
        match self {
            Self::Sprites2d => {
                app.add_systems(Update, (update_player, view::draw_player).chain());
            }
            Self::Billboard3d => {
                app.init_resource::<camera::Orbit>()
                    .add_systems(Startup, camera::spawn_camera)
                    .add_systems(
                        Update,
                        (
                            camera::read_mouse,
                            update_player,
                            camera::follow_player,
                            view::draw_player,
                        )
                            .chain(),
                    );
            }
        }
    }
}

fn load_roster(mut commands: Commands, server: Res<AssetServer>, mode: Res<CharactersPlugin>) {
    commands.insert_resource(RosterHandle(server.load("characters/characters.ron")));
    commands.spawn((
        Name::new("Player"),
        Player::new(matches!(*mode, CharactersPlugin::Billboard3d)),
        Transform::default(),
    ));
}

fn update_player(
    input: Res<ButtonInput<KeyCode>>,
    time: Res<Time>,
    orbit: Option<Res<camera::Orbit>>,
    handle: Res<RosterHandle>,
    rosters: Res<Assets<Roster>>,
    mut player: Query<(&mut Player, &mut Name)>,
) {
    let (Some(roster), Ok((mut player, mut name))) = (rosters.get(&handle.0), player.single_mut())
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
    if name.as_str() != entry.name {
        *name = Name::new(entry.name.clone());
        info!("Selected {} (max health: {})", entry.name, entry.max_health);
    }
    let direction = motion::movement_input(&input, orbit.is_some());
    let world_direction = orbit
        .as_ref()
        .map_or(direction, |orbit| orbit.rotate_move(direction));
    player.update(entry, &input, direction, world_direction, time.delta_secs());
}

#[cfg(test)]
mod tests;
