use bevy::prelude::*;
use p02_rpg_demo::*;

fn hp(app: &App, entity: Entity) -> i32 {
    app.world().get::<Health>(entity).unwrap().current
}

fn set_hp(app: &mut App, entity: Entity, value: i32) {
    app.world_mut().get_mut::<Health>(entity).unwrap().current = value;
}

fn attack(app: &mut App, attacker: Entity, kind: AttackKind) {
    app.world_mut()
        .resource_mut::<Messages<AttackRequest>>()
        .write(AttackRequest { attacker, kind });
}

fn single(app: &mut App, attacker: Entity, target: Entity) {
    attack(app, attacker, AttackKind::Single { target });
}

#[test]
fn spawn_exactly_eight_units() {
    let mut app = build_app(false);
    let mut query = app.world_mut().query::<&Role>();
    let roles: Vec<_> = query.iter(app.world()).copied().collect();
    assert_eq!(roles.len(), 8);
    for (role, count) in [
        (Role::Hero, 1),
        (Role::Minion, 3),
        (Role::Elite, 3),
        (Role::Boss, 1),
    ] {
        assert_eq!(roles.iter().filter(|r| **r == role).count(), count);
    }
}

#[test]
fn boss_single_hits_only_selected_target() {
    for index in 0..7 {
        let mut app = build_app(false);
        let r = app.world().resource::<Roster>().clone();
        let targets: Vec<_> = [r.hero]
            .into_iter()
            .chain(r.minions)
            .chain(r.elites)
            .collect();
        let before: Vec<_> = targets.iter().map(|e| hp(&app, *e)).collect();
        single(&mut app, r.boss, targets[index]);
        app.update();
        for (i, entity) in targets.iter().enumerate() {
            assert_eq!(
                hp(&app, *entity),
                before[i] - if i == index { 15 } else { 0 }
            );
        }
        assert_eq!(hp(&app, r.boss), 60);
    }
}

#[test]
fn boss_aoe_hits_all_seven_other_units() {
    let mut app = build_app(false);
    let r = app.world().resource::<Roster>().clone();
    attack(&mut app, r.boss, AttackKind::ArenaAoe);
    app.update();
    assert_eq!(hp(&app, r.hero), 485);
    for e in r.minions {
        assert_eq!(hp(&app, e), 15);
    }
    for e in r.elites {
        assert_eq!(hp(&app, e), 45);
    }
    assert_eq!(hp(&app, r.boss), 60);
    let log = app.world().resource::<CombatLog>();
    assert_eq!(log.damage.len(), 7);
    let mut expected: Vec<_> = [r.hero]
        .into_iter()
        .chain(r.minions)
        .chain(r.elites)
        .collect();
    expected.sort();
    let mut sounds = log.sounds.clone();
    let mut particles = log.particles.clone();
    sounds.sort();
    particles.sort();
    assert_eq!(sounds, expected);
    assert_eq!(particles, expected);
    app.update();
    let log = app.world().resource::<CombatLog>();
    assert_eq!(log.damage.len(), 7);
    assert_eq!(log.sounds.len(), 7);
    assert_eq!(log.particles.len(), 7);
    assert!(log.deaths.is_empty());
}

#[test]
fn hero_can_attack_each_kind_of_monster() {
    for index in 0..7 {
        let mut app = build_app(false);
        let r = app.world().resource::<Roster>().clone();
        let targets: Vec<_> = r
            .minions
            .into_iter()
            .chain(r.elites)
            .chain([r.boss])
            .collect();
        let target = targets[index];
        let before = hp(&app, target);
        single(&mut app, r.hero, target);
        app.update();
        assert_eq!(hp(&app, target), before - 20);
        assert!(app.world().resource::<CombatLog>().rejected.is_empty());
    }
}

#[test]
fn minions_and_elites_can_only_attack_hero() {
    let mut app = build_app(false);
    let r = app.world().resource::<Roster>().clone();
    for enemy in r.minions.into_iter().chain(r.elites) {
        single(&mut app, enemy, r.hero);
        single(&mut app, enemy, r.boss);
        single(&mut app, enemy, r.minions[0]);
    }
    app.update();
    assert_eq!(hp(&app, r.hero), 479);
    assert_eq!(hp(&app, r.boss), 60);
    assert_eq!(hp(&app, r.minions[0]), 30);
    assert_eq!(app.world().resource::<CombatLog>().rejected.len(), 12);
}

#[test]
fn rejects_self_non_boss_aoe_and_missing_entities() {
    let mut app = build_app(false);
    let r = app.world().resource::<Roster>().clone();
    let missing = app.world_mut().spawn_empty().id();
    app.world_mut().despawn(missing);
    single(&mut app, r.hero, r.hero);
    attack(&mut app, r.hero, AttackKind::ArenaAoe);
    single(&mut app, r.hero, missing);
    single(&mut app, missing, r.hero);
    app.update();
    let log = app.world().resource::<CombatLog>();
    assert_eq!(log.rejected.len(), 4);
    assert!(log.damage.is_empty());
}

#[test]
fn simultaneous_example_and_death_notifications_are_not_repeated() {
    let mut app = build_app(false);
    let r = app.world().resource::<Roster>().clone();
    set_hp(&mut app, r.hero, 100);
    set_hp(&mut app, r.boss, 20);
    for e in r.minions {
        set_hp(&mut app, e, 10);
    }
    for e in r.elites {
        set_hp(&mut app, e, 40);
    }
    single(&mut app, r.hero, r.boss);
    attack(&mut app, r.boss, AttackKind::ArenaAoe);
    for e in r.minions.into_iter().chain(r.elites) {
        single(&mut app, e, r.hero);
    }
    app.update();
    assert_eq!(hp(&app, r.hero), 64);
    let log = app.world().resource::<CombatLog>();
    assert_eq!(log.deaths.len(), 7);
    for entity in r.minions.into_iter().chain(r.elites).chain([r.boss]) {
        let entries: Vec<_> = log
            .deaths
            .iter()
            .filter(|death| death.entity == entity)
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].cause,
            if r.elites.contains(&entity) {
                DeathCause::BossDefeated
            } else {
                DeathCause::Combat
            }
        );
    }
    let history = (log.damage.len(), log.sounds.len(), log.particles.len());
    for e in r.minions.into_iter().chain([r.boss]) {
        assert_eq!(
            *app.world().get::<LifeState>(e).unwrap(),
            LifeState::Dead(DeathCause::Combat)
        );
    }
    for e in r.elites {
        assert_eq!(hp(&app, e), 0);
        assert_eq!(
            *app.world().get::<LifeState>(e).unwrap(),
            LifeState::Dead(DeathCause::BossDefeated)
        );
    }
    single(&mut app, r.boss, r.hero);
    single(&mut app, r.hero, r.minions[0]);
    app.update();
    assert_eq!(hp(&app, r.hero), 64);
    assert_eq!(app.world().resource::<CombatLog>().deaths.len(), 7);
    assert_eq!(app.world().resource::<CombatLog>().rejected.len(), 2);
    let log = app.world().resource::<CombatLog>();
    assert_eq!(
        (log.damage.len(), log.sounds.len(), log.particles.len()),
        history
    );
}

#[test]
fn lethal_attacks_are_order_independent() {
    for reverse in [false, true] {
        let mut app = build_app(false);
        let r = app.world().resource::<Roster>().clone();
        set_hp(&mut app, r.hero, 15);
        set_hp(&mut app, r.boss, 20);
        let mut attacks = [(r.hero, r.boss), (r.boss, r.hero)];
        if reverse {
            attacks.reverse();
        }
        for (attacker, target) in attacks {
            single(&mut app, attacker, target);
        }
        app.update();
        assert_eq!(hp(&app, r.hero), 0);
        assert_eq!(hp(&app, r.boss), 0);
        assert_eq!(app.world().resource::<CombatLog>().deaths.len(), 8);
        assert_eq!(app.world().resource::<CombatLog>().damage.len(), 2);
    }
}

#[test]
fn multiple_attackers_are_aggregated_and_overkill_is_capped() {
    for reverse in [false, true] {
        let mut app = build_app(false);
        let r = app.world().resource::<Roster>().clone();
        set_hp(&mut app, r.hero, 4);
        let mut attackers = [r.minions[0], r.elites[0]];
        if reverse {
            attackers.reverse();
        }
        for attacker in attackers {
            single(&mut app, attacker, r.hero);
        }
        app.update();
        let log = app.world().resource::<CombatLog>();
        assert_eq!(log.damage.len(), 1);
        let hit = &log.damage[0];
        assert_eq!(hit.target, r.hero);
        assert_eq!(hit.actual_damage, 4);
        assert_eq!(hit.remaining_hp, 0);
        attackers.sort();
        assert_eq!(hit.attackers, attackers);
        assert_eq!(log.deaths.len(), 1);
        assert_eq!(log.deaths[0].entity, r.hero);
        assert_eq!(log.deaths[0].cause, DeathCause::Combat);
    }
}

#[test]
fn auto_demo_stops_attacking_after_boss_death() {
    let mut app = build_app(true);
    let r = app.world().resource::<Roster>().clone();
    app.update();
    assert_eq!(hp(&app, r.hero), 464);
    app.update();
    assert_eq!(hp(&app, r.hero), 413);
    app.update();
    assert_eq!(hp(&app, r.hero), 377);
    assert_eq!(app.world().resource::<CombatLog>().deaths.len(), 7);
    let damage_count = app.world().resource::<CombatLog>().damage.len();
    app.update();
    assert_eq!(hp(&app, r.hero), 377);
    assert_eq!(
        app.world().resource::<CombatLog>().damage.len(),
        damage_count
    );
}
