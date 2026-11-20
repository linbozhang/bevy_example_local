use bevy::prelude::*;
use p02_rpg_demo::*;

fn main() {
    let mut app = build_app(true);
    let roster = app.world().resource::<Roster>().clone();
    println!("RPG 消息流水线演示：1 英雄、3 小怪、3 精英、1 Boss");
    println!("每次 update 是一回合；Boss 第 2 回合使用 AOE，第 3 回合被英雄击杀。\n");

    for _ in 0..4 {
        let damage_start = app.world().resource::<CombatLog>().damage.len();
        let death_start = app.world().resource::<CombatLog>().deaths.len();
        app.update();
        let world = app.world();
        let log = world.resource::<CombatLog>();
        println!("=== 回合 {} ===", world.resource::<Turn>().0);
        for hit in &log.damage[damage_start..] {
            let name = world.get::<Name>(hit.target).unwrap();
            println!(
                "{name}: 受到 {} 点伤害，剩余 HP {}（声音 + 受击特效）",
                hit.actual_damage, hit.remaining_hp
            );
        }
        for death in &log.deaths[death_start..] {
            let name = world.get::<Name>(death.entity).unwrap();
            println!("{name}: 死亡，原因 {:?}", death.cause);
        }
        if log.damage.len() == damage_start {
            println!("没有新的伤害：死亡单位不再发起攻击。");
        }
        println!();
    }

    println!("=== 最终状态 ===");
    for entity in [roster.hero]
        .into_iter()
        .chain(roster.minions)
        .chain(roster.elites)
        .chain([roster.boss])
    {
        let world = app.world();
        println!(
            "{}: HP {}/{}, {:?}",
            world.get::<Name>(entity).unwrap(),
            world.get::<Health>(entity).unwrap().current,
            world.get::<Health>(entity).unwrap().max,
            world.get::<LifeState>(entity).unwrap()
        );
    }
    // 教学版保留尸体供检查；真实游戏可在死亡动画结束后再 despawn。
}
