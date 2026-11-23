use bevy::{light::NotShadowCaster, math::Affine2, prelude::*};

use crate::{
    characters::{Action, Character, FollowCamera, Roster, RosterHandle, ground_position},
    map::generate::{TILE_SIZE, depth_from_feet},
};

use super::protocol::{Identity, Peer, Pose};

#[derive(Resource, Default)]
pub(super) struct Snapshot(pub Vec<Peer>);

#[derive(Resource)]
pub(super) struct ViewMode {
    pub sprites_2d: bool,
}

/// A visual-only replica, deliberately not a `Player`: no input, camera, collisions or magic.
#[derive(Component)]
pub(super) struct RemotePlayer {
    id: Identity,
    target: Pose,
    position: Vec2,
    lift: f32,
    walk_time: f64,
    drawn_selection: Option<usize>,
}

impl RemotePlayer {
    fn new(peer: Peer) -> Self {
        Self {
            id: peer.id,
            target: peer.pose,
            position: peer.pose.position.into(),
            lift: peer.pose.lift,
            walk_time: 0.0,
            drawn_selection: None,
        }
    }

    fn advance(&mut self, dt: f32) {
        let target = Vec2::from_array(self.target.position);
        let distance = self.position.distance(target);
        let weight = 1.0 - (-16.0 * dt).exp();
        if distance > 0.5 {
            self.position = self.position.lerp(target, weight);
            self.walk_time += f64::from(dt);
        } else {
            self.position = target;
            self.walk_time = 0.0;
        }
        self.lift += (self.target.lift - self.lift) * weight;
        if (self.lift - self.target.lift).abs() < 0.005 {
            self.lift = self.target.lift;
        }
    }
}

pub(super) fn sync_players(
    mut commands: Commands,
    snapshot: Res<Snapshot>,
    mut existing: Query<(Entity, &mut RemotePlayer)>,
) {
    if !snapshot.is_changed() {
        return;
    }
    // The snapshot is bounded to 15 peers, so linear scans avoid allocating a second map.
    let mut present = [false; super::protocol::MAX_CLIENTS];
    for (entity, mut remote) in &mut existing {
        if let Some(index) = snapshot.0.iter().position(|peer| peer.id == remote.id) {
            present[index] = true;
            remote.target = snapshot.0[index].pose;
        } else {
            commands.entity(entity).despawn();
        }
    }
    for (peer, already_present) in snapshot.0.iter().zip(present) {
        if !already_present {
            commands.spawn((
                Name::new(format!("Remote player {:032x}", peer.id)),
                RemotePlayer::new(*peer),
                Transform::default(),
            ));
        }
    }
}

fn animation_frame(entry: &Character, direction: Vec2, walk_time: f64) -> UVec2 {
    let clip = &entry.animations[&Action::Walk];
    let facing_row = if direction.x.abs() > direction.y.abs() {
        if direction.x < 0.0 { 1 } else { 3 }
    } else if direction.y < 0.0 {
        2
    } else {
        0
    };
    UVec2::new(
        ((walk_time / f64::from(clip.frame_time)) % f64::from(clip.frame_count)) as u32,
        clip.start_row + if clip.directional { facing_row } else { 0 },
    )
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn draw_players(
    mut commands: Commands,
    time: Res<Time>,
    mode: Res<ViewMode>,
    handle: Res<RosterHandle>,
    rosters: Res<Assets<Roster>>,
    server: Res<AssetServer>,
    mut layouts: ResMut<Assets<TextureAtlasLayout>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    camera: Query<&Transform, (With<FollowCamera>, Without<RemotePlayer>)>,
    mut players: Query<(
        Entity,
        &mut RemotePlayer,
        &mut Transform,
        Option<&mut Sprite>,
        Option<&MeshMaterial3d<StandardMaterial>>,
    )>,
) {
    let Some(roster) = rosters.get(&handle.0) else {
        return;
    };
    let rotation = if mode.sprites_2d {
        Quat::IDENTITY
    } else {
        let Ok(camera) = camera.single() else { return };
        camera.rotation
    };
    for (entity, mut remote, mut transform, sprite, material) in &mut players {
        remote.advance(time.delta_secs());
        let mut facing = Vec2::from_array(remote.target.facing);
        if !mode.sprites_2d {
            // Project world planar facing into the observer camera's horizontal axes.
            let forward = rotation * Vec3::NEG_Z;
            let right = rotation * Vec3::X;
            let world = Vec3::new(facing.x, 0.0, -facing.y);
            facing = Vec2::new(
                world.dot(right),
                world.dot(forward) / (forward.xz().length().max(0.001)),
            );
        }
        let selected = usize::from(remote.target.selected);
        let selected = if selected < roster.characters.len() {
            selected
        } else {
            0
        };
        let entry = &roster.characters[selected];
        if remote.drawn_selection != Some(selected) {
            remote.walk_time = 0.0;
        }
        let frame = animation_frame(entry, facing, remote.walk_time);
        let height = remote.lift * 20.0;
        let changed = remote.drawn_selection != Some(selected);
        if mode.sprites_2d {
            transform.translation = Vec3::new(
                remote.position.x,
                remote.position.y + height + entry.tile_size as f32 / 3.0,
                depth_from_feet(remote.position.y) + 0.00015,
            );
            let index = (frame.y * entry.atlas_columns + frame.x) as usize;
            if changed {
                let layout = layouts.add(TextureAtlasLayout::from_grid(
                    UVec2::splat(entry.tile_size),
                    entry.atlas_columns,
                    entry.atlas_rows,
                    None,
                    None,
                ));
                commands.entity(entity).insert(Sprite::from_atlas_image(
                    server.load(&entry.texture_path),
                    TextureAtlas { layout, index },
                ));
            } else if let Some(mut sprite) = sprite
                && let Some(atlas) = &mut sprite.texture_atlas
                && atlas.index != index
            {
                atlas.index = index;
            }
        } else {
            let size = entry.tile_size as f32 / TILE_SIZE;
            transform.rotation = rotation;
            transform.scale = Vec3::splat(size);
            transform.translation = ground_position(remote.position)
                + Vec3::Y * (0.021 + height / TILE_SIZE)
                + rotation * Vec3::Y * (size / 3.0);
            let cells = Vec2::new(entry.atlas_columns as f32, entry.atlas_rows as f32);
            let uv =
                Affine2::from_scale_angle_translation(cells.recip(), 0.0, frame.as_vec2() / cells);
            if let Some(material) = material {
                if let Some(mat) = materials.get(&material.0)
                    && (changed || mat.uv_transform != uv)
                    && let Some(mut mat) = materials.get_mut(&material.0)
                {
                    if changed {
                        mat.base_color_texture = Some(server.load(&entry.texture_path));
                    }
                    mat.uv_transform = uv;
                }
            } else {
                commands.entity(entity).insert((
                    Mesh3d(meshes.add(Rectangle::new(1.0, 1.0))),
                    MeshMaterial3d(materials.add(StandardMaterial {
                        base_color_texture: Some(server.load(&entry.texture_path)),
                        uv_transform: uv,
                        alpha_mode: AlphaMode::Mask(0.5),
                        unlit: true,
                        ..default()
                    })),
                    NotShadowCaster,
                ));
            }
        }
        remote.drawn_selection = Some(selected);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::characters::Player;

    fn peer(x: f32) -> Peer {
        Peer {
            id: 1,
            pose: Pose {
                position: [x, 0.0],
                facing: [1.0, 0.0],
                lift: 0.0,
                selected: 0,
            },
        }
    }

    #[test]
    fn replicas_use_map_depth_in_2d_and_camera_facing_ground_coordinates_in_3d() {
        for sprites_2d in [true, false] {
            let mut app = App::new();
            app.add_plugins((bevy::app::TaskPoolPlugin::default(), AssetPlugin::default()))
                .init_asset::<Roster>()
                .init_asset::<Image>()
                .init_asset::<Mesh>()
                .init_asset::<StandardMaterial>()
                .init_asset::<TextureAtlasLayout>()
                .init_resource::<Time>()
                .insert_resource(ViewMode { sprites_2d })
                .insert_resource(Snapshot(vec![peer(10.0)]))
                .add_systems(Update, (sync_players, draw_players).chain());
            let roster: Roster =
                ron::from_str(include_str!("../assets/characters/characters.ron")).unwrap();
            let size_px = roster.characters[0].tile_size as f32;
            let world = app.world_mut();
            let handle = world.resource_mut::<Assets<Roster>>().add(roster);
            world.insert_resource(RosterHandle(handle));
            let rotation = Quat::from_rotation_y(0.7);
            world.spawn((FollowCamera, Transform::from_rotation(rotation)));
            // A client with an unexpected roster index should fall back to the first character.
            world.resource_mut::<Snapshot>().0[0].pose.selected = u8::MAX;
            app.update();
            let world = app.world_mut();
            let (entity, transform) = world
                .query_filtered::<(Entity, &Transform), With<RemotePlayer>>()
                .single(world)
                .unwrap();
            let xy = Vec2::new(10.0, 0.0);
            if sprites_2d {
                assert!(world.get::<Sprite>(entity).unwrap().texture_atlas.is_some());
                assert_eq!(
                    transform.translation,
                    Vec3::new(xy.x, size_px / 3.0, depth_from_feet(xy.y) + 0.00015)
                );
            } else {
                assert!(
                    world
                        .get::<MeshMaterial3d<StandardMaterial>>(entity)
                        .is_some()
                );
                assert_eq!(transform.rotation, rotation);
                assert!(transform.translation.abs_diff_eq(
                    ground_position(xy)
                        + 0.021 * Vec3::Y
                        + rotation * Vec3::Y * (size_px / TILE_SIZE / 3.0),
                    1e-6
                ));
            }
        }
    }

    #[test]
    fn peers_spawn_update_in_place_and_despawn_without_a_player_component() {
        let mut app = App::new();
        app.insert_resource(Snapshot(vec![peer(10.0)]))
            .add_systems(Update, sync_players);
        app.update();
        let world = app.world_mut();
        let (entity, remote) = world
            .query::<(Entity, &RemotePlayer)>()
            .single(world)
            .unwrap();
        assert_eq!(remote.position, Vec2::new(10.0, 0.0));
        assert!(world.get::<Player>(entity).is_none());
        world.resource_mut::<Snapshot>().0 = vec![peer(42.0)];
        app.update();
        let mut remote = app.world_mut().get_mut::<RemotePlayer>(entity).unwrap();
        remote.advance(0.016);
        assert!((10.0..42.0).contains(&remote.position.x));
        for _ in 0..90 {
            remote.advance(0.016);
        }
        assert_eq!(remote.position.x, 42.0);
        app.world_mut().resource_mut::<Snapshot>().0.clear();
        app.update();
        assert!(app.world().get::<RemotePlayer>(entity).is_none());
    }
}
