//! Chapters 12–14, adapted to a trusted localhost TCP transport rather than SpacetimeDB.
mod client;
#[allow(dead_code)] // The server binary also builds this codec, using the other half.
mod protocol;
mod remote;

use std::{io, path::Path, sync::Mutex};

use bevy::{prelude::*, transform::TransformSystems};

use crate::{
    characters::{Orbit, Player},
    game_state::GameState,
};
use client::Connection;
use protocol::Pose;

pub(crate) struct NetworkPlugin {
    seed: u64,
    sprites_2d: bool,
    connection: Mutex<Option<Connection>>,
}

impl NetworkPlugin {
    /// Complete the handshake before either map mode runs its startup generation.
    pub(crate) fn connect(sprites_2d: bool, identity_path: impl AsRef<Path>) -> io::Result<Self> {
        let (seed, connection) = Connection::connect(identity_path.as_ref())?;
        Ok(Self {
            seed,
            sprites_2d,
            connection: Mutex::new(Some(connection)),
        })
    }

    pub(crate) fn world_seed(&self) -> u64 {
        self.seed
    }
}

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        let connection = self
            .connection
            .lock()
            .unwrap()
            .take()
            .expect("add the network plugin once");
        app.insert_resource(connection)
            .insert_resource(remote::ViewMode {
                sprites_2d: self.sprites_2d,
            })
            .init_resource::<remote::Snapshot>()
            .add_systems(
                PostUpdate,
                (
                    publish_and_receive,
                    remote::sync_players,
                    remote::draw_players,
                )
                    .chain()
                    .before(TransformSystems::Propagate),
            );
    }
}

fn publish_and_receive(
    mut commands: Commands,
    connection: Option<Res<Connection>>,
    state: Res<State<GameState>>,
    mut snapshot: ResMut<remote::Snapshot>,
    local: Query<&Player>,
    orbit: Option<Res<Orbit>>,
) {
    let Some(connection) = connection else { return };
    let in_world = matches!(state.get(), GameState::Playing | GameState::Paused);
    let pose = if in_world {
        local.single().ok().map(|player| {
            // The local 3D player's facing is camera-relative. Put it on the wire in world XY.
            let facing = player.facing_direction();
            let facing = orbit
                .as_ref()
                .map_or(facing, |orbit| orbit.rotate_move(facing));
            Pose {
                position: player.position.to_array(),
                facing: facing.to_array(),
                lift: player.lift,
                selected: player.selected as u8,
            }
        })
    } else {
        None
    };
    connection.set_pose(pose);
    match connection.receive() {
        Some(Ok(peers)) => snapshot.0 = peers,
        Some(Err(error)) => {
            snapshot.0.clear();
            commands.remove_resource::<Connection>();
            warn!(
                "Disconnected from local world ({error}); remotes removed. Restart to reconnect."
            );
        }
        None => {}
    }
    if pose.is_none() && !snapshot.0.is_empty() {
        snapshot.0.clear();
    }
}
