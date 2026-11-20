# p02_rpg_demo：RPG 消息与死亡结算

无窗口的 Bevy ECS 教学工程。没有模型、输入、真实音频或粒子资产；先用控制台日志把战斗规则和调度学清楚。使用工作区统一的 Bevy 版本，不修改官方 examples。

## 运行

在仓库根目录执行：

```sh
cargo run -p p02_rpg_demo
cargo test -p p02_rpg_demo
```

`main.rs` 手动调用四次 `App::update()`，每次是一回合，随后退出，不依赖真实时间。角色攻击间隔用回合数表示。

## 规则

场内固定生成 1 英雄、3 普通小怪、3 精英怪、1 Boss。

- 英雄可单体攻击所有怪物；演示中默认瞄准 Boss。
- 普通小怪、精英怪只能攻击英雄。
- Boss 单体攻击仅伤害指定目标，允许选择英雄、普通小怪或精英怪；演示中瞄准英雄。
- Boss 全场 AOE 伤害英雄及所有存活普通/精英怪，不伤害 Boss 自己。这里不做距离判定。
- Boss 死亡，所有存活普通/精英怪直接死亡，不走巨额伤害或减伤流程。
- 同回合同时结算：按回合开始的存活状态验证攻击，汇总每个目标的伤害，再统一扣血。即使攻击者本回合死亡，其已验证的攻击仍生效。允许同归于尽。
- 每个目标每回合发一条汇总 `DamageApplied`；过量伤害裁剪到实际剩余血量。每个单位只发一次 `UnitDied`。

| 单位 | 初始 HP | 攻击力 | 单体攻击间隔 |
|---|---:|---:|---:|
| 英雄 | 500 | 20 | 1 回合 |
| 普通小怪 | 30 | 2 | 1 回合 |
| 精英怪 | 60 | 5 | 1 回合 |
| Boss | 60 | 15 | 1 回合 |

Boss 另有 AOE 冷却：第 2 回合首次释放，此后每 2 回合一次。单体和 AOE 可在同回合发生。

### 默认演示结果

1. 第 1 回合：英雄 HP 464，Boss HP 40。
2. 第 2 回合：Boss 单体 + AOE；英雄 HP 413，Boss HP 20，小怪 HP 15，精英 HP 45。
3. 第 3 回合：英雄击杀 Boss；Boss 和小怪已经发出的攻击仍生效，英雄 HP 377；六只普通/精英怪连锁死亡。
4. 第 4 回合：死亡怪物不再攻击；英雄仍以 Boss 为目标，其攻击请求因目标已死亡被拒绝，不再产生伤害。

## 按这个顺序读代码

- `src/main.rs`：创建应用、推进回合、打印结果。
- `src/lib.rs`：组件与消息 → `CombatPlugin` 的阶段排序 → 攻击意图 → 统一目标验证 → 汇总扣血 → 死亡与连锁死亡 → 两个独立反馈 reader。
- `tests/combat.rs`：关闭 AI 后直接写入攻击请求，验证同一条完整流水线，而不是绕过系统手动扣血。

```text
CombatSet::Intent
  single_attacks / boss_aoe_attack
      ↓ AttackRequest（攻击意图）
CombatSet::ResolveTargets
  resolve_attack_targets
      ↓ DamageRequest（验证通过、展开具体目标）
CombatSet::ApplyDamage
  apply_damage
      ↓ DamageApplied（每个目标的实际扣血结果）
CombatSet::ResolveDeaths
  resolve_deaths
      ↓ UnitDied（包括 Boss 连锁死亡）
CombatSet::Feedback
  record_results / play_hit_sound / spawn_hit_effect
```

组与组通过 `SystemSet + chain()` 明确排序。组内不规定先后，Bevy 按资源访问冲突决定能否并行。反馈系统都写同一个教学用 `CombatLog`，所以当前实现不会彼此并行，但独立 reader 仍各自收到同一份消息。

## 为什么保留尸体？

死亡状态是预先存在的 `LifeState` 组件，直接从 `Alive` 改成 `Dead(cause)`；血量立即变为 0。战斗阶段不使用 `Commands` 插入标记或立即删除实体，避免延迟可见性和反馈阶段查询不到死者资料的问题。

教学版保留八个实体供最终状态检查，不实现尸体清理、复活、掉落或胜负 UI。真实游戏可在死亡动画结束后清理实体；反馈消息应携带清理之后仍需要的数据。

`CombatLog` 保存整场历史仅为观察和测试；长期运行的游戏应限制日志大小。

## 练习方向

1. 修改英雄的 `Target`，观察只改变目标后，其余伤害流水线无需改变。
2. 给普通小怪添加护甲，在汇总扣血前统一结算，确认零伤害不播放受击音效。
3. 将 `ArenaAoe` 改为带中心和半径的攻击，加入位置组件与距离筛选。
4. 用真实音效、粒子和 UI 替换日志反馈，不让它们直接读取 `AttackRequest`。
