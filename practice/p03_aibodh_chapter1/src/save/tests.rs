use super::{storage::tests::example, *};
use crate::inventory::ItemKind;

fn generated_world(third_person: bool) -> (World, Entity, Snapshot) {
    let mut world = World::new();
    let roster: Roster =
        ron::de::from_bytes(include_bytes!("../assets/characters/characters.ron")).unwrap();
    let mut saved = example();
    saved.player.character = roster.characters[1].name.clone();
    let mut assets = Assets::<Roster>::default();
    world.insert_resource(RosterHandle(assets.add(roster)));
    world.insert_resource(assets);
    world.insert_resource(MapSeed(saved.seed));
    world.insert_resource(CollisionMap::from_tiles([]));
    world.init_resource::<Inventory>();
    let player = world
        .spawn((
            Player::new(third_person),
            Health {
                current: 100.0,
                max: 100.0,
            },
        ))
        .id();
    (world, player, saved)
}

#[test]
fn restore_reuses_generated_actors_and_props_in_both_modes() {
    for third_person in [false, true] {
        let (mut world, player, saved) = generated_world(third_person);
        let spawn_enemy = |world: &mut World, index: usize| {
            let mut enemy = Enemy::default();
            enemy.spawn_index = index;
            world.spawn((enemy, Transform::default())).id()
        };
        let defeated = spawn_enemy(&mut world, 0);
        let survivor = spawn_enemy(&mut world, 1);
        let collected = world.spawn(Pickable::new(ItemKind::Fern, Vec2::ZERO)).id();
        let remaining = world
            .spawn(Pickable::new(
                ItemKind::Mushroom,
                Vec2::from_array(saved.pickups[0].position),
            ))
            .id();
        let expected_player_position = Vec2::from_array(saved.player.position);
        let expected_enemy_position = Vec2::from_array(saved.enemies[&1].position);
        restore(&mut world, saved).unwrap();

        let player_state = world.get::<Player>(player).unwrap();
        assert_eq!(player_state.position, expected_player_position);
        assert_eq!(player_state.selected, 1);
        assert_eq!(world.get::<Health>(player).unwrap().current, 13.0);
        assert_eq!(
            world.get::<Enemy>(survivor).unwrap().position,
            expected_enemy_position
        );
        assert_eq!(world.get::<Health>(survivor).unwrap().current, 4.0);
        assert!(world.get::<Transform>(survivor).is_some());
        assert!(world.get_entity(defeated).is_err());
        assert!(world.get_entity(collected).is_err());
        assert!(world.get::<Pickable>(remaining).is_some());
        assert_eq!(world.resource::<Inventory>().0[&ItemKind::Herb], 3);
        let recaptured = capture(&mut world).unwrap();
        assert_eq!(recaptured.seed, world.resource::<MapSeed>().0);
        assert_eq!(recaptured.enemies.len(), 1);
        assert_eq!(recaptured.pickups.len(), 1);
    }
}

#[test]
fn restore_rejects_mismatched_world_without_partial_update() {
    let (mut world, player, mut saved) = generated_world(false);
    saved.enemies.clear();
    let fresh_pickup = world.spawn(Pickable::new(ItemKind::Herb, Vec2::ZERO)).id();
    assert_eq!(
        restore(&mut world, saved).unwrap_err(),
        "saved pickups do not match generated terrain"
    );
    assert_eq!(world.get::<Player>(player).unwrap().selected, 0);
    assert_eq!(world.get::<Health>(player).unwrap().current, 100.0);
    assert!(world.get::<Pickable>(fresh_pickup).is_some());
    assert!(world.resource::<Inventory>().0.is_empty());
}
