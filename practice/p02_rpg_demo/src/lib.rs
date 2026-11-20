//! 无窗口、回合制 ECS 战斗：每次 App::update() 是一个战斗回合。
//! 意图 → 目标验证 → 汇总扣血 → 死亡/连锁死亡 → 反馈。
use std::collections::BTreeMap;

use bevy::prelude::*;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Hero,
    Minion,
    Elite,
    Boss,
}

#[derive(Component, Debug)]
pub struct Health {
    pub current: i32,
    pub max: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeathCause {
    Combat,
    BossDefeated,
}

// 始终存在的组件：直接修改状态，不依赖 Commands 的延迟插入。
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeState {
    Alive,
    Dead(DeathCause),
}

#[derive(Component)]
pub struct AttackPower(pub i32);

#[derive(Component)]
pub struct Target(pub Entity);

// 回合冷却，而非墙上时钟：运行和测试都可确定地复现。
#[derive(Component)]
pub struct SingleAttackCooldown {
    pub every: u32,
    pub next_turn: u32,
}

#[derive(Component)]
pub struct AoeCooldown {
    pub every: u32,
    pub next_turn: u32,
}

#[derive(Resource, Default)]
pub struct Turn(pub u32);

// 教学演示用资源，方便入口和测试找到最初的八个实体。
#[derive(Resource, Clone)]
pub struct Roster {
    pub hero: Entity,
    pub minions: [Entity; 3],
    pub elites: [Entity; 3],
    pub boss: Entity,
}

#[derive(Debug, Clone, Copy)]
pub enum AttackKind {
    Single { target: Entity },
    ArenaAoe,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct AttackRequest {
    pub attacker: Entity,
    pub kind: AttackKind,
}

// 只由验证系统生成，不向输入层暴露“绕过验证直接扣血”的接口。
#[derive(Message)]
struct DamageRequest {
    attacker: Entity,
    target: Entity,
    amount: i32,
}

// 每个目标每回合一个结果，避免依赖攻击消息到达顺序。
#[derive(Message, Debug, Clone)]
pub struct DamageApplied {
    pub target: Entity,
    pub attackers: Vec<Entity>,
    pub actual_damage: i32,
    pub remaining_hp: i32,
}

#[derive(Message, Debug, Clone, Copy)]
pub struct UnitDied {
    pub entity: Entity,
    pub cause: DeathCause,
}

// 保存历史便于演示/断言；这不是长期运行游戏的日志存储方案。
#[derive(Resource, Default)]
pub struct CombatLog {
    pub damage: Vec<DamageApplied>,
    pub deaths: Vec<UnitDied>,
    pub sounds: Vec<Entity>,
    pub particles: Vec<Entity>,
    pub rejected: Vec<String>,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombatSet {
    Intent,
    ResolveTargets,
    ApplyDamage,
    ResolveDeaths,
    Feedback,
}

pub struct CombatPlugin;

impl Plugin for CombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Turn>()
            .init_resource::<CombatLog>()
            .add_message::<AttackRequest>()
            .add_message::<DamageRequest>()
            .add_message::<DamageApplied>()
            .add_message::<UnitDied>()
            .configure_sets(
                Update,
                (
                    CombatSet::Intent,
                    CombatSet::ResolveTargets,
                    CombatSet::ApplyDamage,
                    CombatSet::ResolveDeaths,
                    CombatSet::Feedback,
                )
                    .chain(),
            )
            .add_systems(Update, begin_turn.before(CombatSet::Intent))
            .add_systems(
                Update,
                resolve_attack_targets.in_set(CombatSet::ResolveTargets),
            )
            .add_systems(Update, apply_damage.in_set(CombatSet::ApplyDamage))
            .add_systems(Update, resolve_deaths.in_set(CombatSet::ResolveDeaths))
            .add_systems(
                Update,
                (record_results, play_hit_sound, spawn_hit_effect).in_set(CombatSet::Feedback),
            );
    }
}

// AI 可关掉：测试直接写 AttackRequest，仍走完整结算流水线。
pub struct AutoAttackPlugin;

impl Plugin for AutoAttackPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (single_attacks, boss_aoe_attack).in_set(CombatSet::Intent),
        );
    }
}

fn begin_turn(mut turn: ResMut<Turn>) {
    turn.0 += 1;
}

fn single_attacks(
    turn: Res<Turn>,
    mut units: Query<(
        Entity,
        &Health,
        &LifeState,
        &Target,
        &mut SingleAttackCooldown,
    )>,
    mut writer: MessageWriter<AttackRequest>,
) {
    for (attacker, hp, life, target, mut cooldown) in &mut units {
        if *life == LifeState::Alive && hp.current > 0 && turn.0 >= cooldown.next_turn {
            writer.write(AttackRequest {
                attacker,
                kind: AttackKind::Single { target: target.0 },
            });
            cooldown.next_turn = turn.0 + cooldown.every.max(1);
        }
    }
}

fn boss_aoe_attack(
    turn: Res<Turn>,
    mut units: Query<(Entity, &Role, &Health, &LifeState, &mut AoeCooldown)>,
    mut writer: MessageWriter<AttackRequest>,
) {
    for (attacker, role, hp, life, mut cooldown) in &mut units {
        if *role == Role::Boss
            && *life == LifeState::Alive
            && hp.current > 0
            && turn.0 >= cooldown.next_turn
        {
            writer.write(AttackRequest {
                attacker,
                kind: AttackKind::ArenaAoe,
            });
            cooldown.next_turn = turn.0 + cooldown.every.max(1);
        }
    }
}

pub fn can_single_attack(attacker: Role, target: Role) -> bool {
    match attacker {
        Role::Hero => matches!(target, Role::Minion | Role::Elite | Role::Boss),
        Role::Minion | Role::Elite => target == Role::Hero,
        Role::Boss => matches!(target, Role::Hero | Role::Minion | Role::Elite),
    }
}

fn resolve_attack_targets(
    mut requests: MessageReader<AttackRequest>,
    units: Query<(Entity, &Role, &Health, &LifeState, &AttackPower)>,
    mut damage: MessageWriter<DamageRequest>,
    mut log: ResMut<CombatLog>,
) {
    for request in requests.read() {
        let Ok((_, role, hp, life, power)) = units.get(request.attacker) else {
            log.rejected.push("攻击者不存在".into());
            continue;
        };
        if *life != LifeState::Alive || hp.current <= 0 || power.0 <= 0 {
            log.rejected.push("攻击者已死亡或攻击力无效".into());
            continue;
        }
        match request.kind {
            AttackKind::Single { target } => {
                let Ok((_, target_role, target_hp, target_life, _)) = units.get(target) else {
                    log.rejected.push("目标不存在".into());
                    continue;
                };
                if target == request.attacker
                    || !can_single_attack(*role, *target_role)
                    || *target_life != LifeState::Alive
                    || target_hp.current <= 0
                {
                    log.rejected.push("单体攻击目标不合法或已死亡".into());
                    continue;
                }
                damage.write(DamageRequest {
                    attacker: request.attacker,
                    target,
                    amount: power.0,
                });
            }
            AttackKind::ArenaAoe => {
                if *role != Role::Boss {
                    log.rejected.push("只有 Boss 可以使用全场 AOE".into());
                    continue;
                }
                for (target, target_role, target_hp, target_life, _) in &units {
                    if target != request.attacker
                        && matches!(target_role, Role::Hero | Role::Minion | Role::Elite)
                        && *target_life == LifeState::Alive
                        && target_hp.current > 0
                    {
                        damage.write(DamageRequest {
                            attacker: request.attacker,
                            target,
                            amount: power.0,
                        });
                    }
                }
            }
        }
    }
}

fn apply_damage(
    mut requests: MessageReader<DamageRequest>,
    mut units: Query<(&mut Health, &LifeState)>,
    mut results: MessageWriter<DamageApplied>,
) {
    let mut totals: BTreeMap<Entity, (i32, Vec<Entity>)> = BTreeMap::new();
    for request in requests.read() {
        let (total, attackers) = totals.entry(request.target).or_default();
        *total = total.saturating_add(request.amount);
        attackers.push(request.attacker);
    }
    for (target, (total, mut attackers)) in totals {
        let Ok((mut hp, life)) = units.get_mut(target) else {
            continue;
        };
        if *life != LifeState::Alive || hp.current <= 0 {
            continue;
        }
        let actual_damage = total.min(hp.current);
        hp.current -= actual_damage;
        attackers.sort();
        results.write(DamageApplied {
            target,
            attackers,
            actual_damage,
            remaining_hp: hp.current,
        });
    }
    // 不在扣血中途重新检查攻击者血量：本回合已验证的攻击全部兑现。
}

fn resolve_deaths(
    mut units: Query<(Entity, &Role, &mut Health, &mut LifeState)>,
    mut deaths: MessageWriter<UnitDied>,
) {
    let boss_dead = units.iter().any(|(_, role, hp, life)| {
        *role == Role::Boss && (hp.current <= 0 || *life != LifeState::Alive)
    });
    for (entity, role, mut hp, mut life) in &mut units {
        if *life != LifeState::Alive {
            continue;
        }
        // 已被普通伤害击杀的单位保留 Combat 原因，不重复报告连锁死亡。
        let cause = if hp.current <= 0 {
            Some(DeathCause::Combat)
        } else if boss_dead && matches!(role, Role::Minion | Role::Elite) {
            Some(DeathCause::BossDefeated)
        } else {
            None
        };
        if let Some(cause) = cause {
            hp.current = 0;
            *life = LifeState::Dead(cause);
            deaths.write(UnitDied { entity, cause });
        }
    }
}

fn record_results(
    mut damage: MessageReader<DamageApplied>,
    mut deaths: MessageReader<UnitDied>,
    mut log: ResMut<CombatLog>,
) {
    log.damage.extend(damage.read().cloned());
    log.deaths.extend(deaths.read().copied());
}

// 两个独立 reader 都能看到同一条结果；这里记录反馈，不加载真实音频/粒子资产。
fn play_hit_sound(mut reader: MessageReader<DamageApplied>, mut log: ResMut<CombatLog>) {
    log.sounds.extend(reader.read().map(|hit| hit.target));
}

fn spawn_hit_effect(mut reader: MessageReader<DamageApplied>, mut log: ResMut<CombatLog>) {
    log.particles.extend(reader.read().map(|hit| hit.target));
}

pub fn build_app(auto_attack: bool) -> App {
    let mut app = App::new();
    app.add_plugins((MinimalPlugins, CombatPlugin));
    if auto_attack {
        app.add_plugins(AutoAttackPlugin);
    }
    let roster = spawn_roster(app.world_mut());
    app.insert_resource(roster);
    app
}

fn spawn_roster(world: &mut World) -> Roster {
    let mut spawn = |name: String, role, hp, power| {
        world
            .spawn((
                Name::new(name),
                role,
                Health {
                    current: hp,
                    max: hp,
                },
                LifeState::Alive,
                AttackPower(power),
                SingleAttackCooldown {
                    every: 1,
                    next_turn: 1,
                },
            ))
            .id()
    };
    let hero = spawn("英雄".into(), Role::Hero, 500, 20);
    let minions = std::array::from_fn(|i| spawn(format!("小怪 {}", i + 1), Role::Minion, 30, 2));
    let elites = std::array::from_fn(|i| spawn(format!("精英 {}", i + 1), Role::Elite, 60, 5));
    let boss = spawn("Boss".into(), Role::Boss, 60, 15);
    world.entity_mut(hero).insert(Target(boss));
    for enemy in minions.into_iter().chain(elites).chain([boss]) {
        world.entity_mut(enemy).insert(Target(hero));
    }
    world.entity_mut(boss).insert(AoeCooldown {
        every: 2,
        next_turn: 2,
    });
    Roster {
        hero,
        minions,
        elites,
        boss,
    }
}
