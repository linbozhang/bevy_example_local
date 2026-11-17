# 01 · App 与 ECS 内核（Stage 1 导读）

> 本篇所有代码都摘自/对齐你本地 fork（Bevy 0.20）的官方 examples，可以放心照抄风格。
> 配套练习：`practice/p01_first_scene`（TODO 清单在源码注释里）。

## 1. Unity → Bevy 概念映射

| Unity | Bevy 0.20 | 备注 |
|---|---|---|
| GameObject | `Entity` | 纯 ID（整数代际句柄），没有任何内建数据 |
| Component（Transform 等） | `Component`（struct） | Unity 继承 MonoBehaviour；Bevy 是**任意数量的裸 struct 叠在实体上** |
| MonoBehaviour.Update() | `System`（函数） | 逻辑不在组件里，是全局函数按「能查到什么数据」分工 |
| Prefab / Scene | `Scene` / gltf / BSN | 见 Stage 2；0.20 新增 BSN 场景 DSL |
| ScriptableObject | `Resource` | 全局单例数据，任何 system 可读写 |
| C# event / SendMessage | `Message`（队列）+ `Observer`（触发式） | 两种机制，见 §7 |
| `Time.deltaTime` | `time.delta_secs()` | |
| `Camera.main` | `Query<&Camera>` / `Camera3d` 查询 | 没有 main 相机概念，查询即所得 |

核心转变一句话：**Unity 是「对象拥有行为」，Bevy 是「行为（system）按数据形状（query）批量处理对象」**。

## 2. App：一切从这开始

```rust
// examples/hello_world.rs —— 最小 Bevy 程序
use bevy::prelude::*;

fn main() {
    App::new().add_systems(Startup, hello_world_system).run();
}

fn hello_world_system() {
    println!("Hello, World!");
}
```

- `App` ≈ 引擎实例 + 场景 + 调度器的三位一体
- `add_systems(Startup, ...)`：把函数注册到 **Startup** 调度（游戏开始前跑一次）
- `.run()`：进入主循环。每帧依次执行各 schedule 里的 systems

真实项目第一步都是装插件组：

```rust
App::new()
    .add_plugins(DefaultPlugins)          // 窗口/渲染/输入/音频/资源... 引擎主体
    .add_systems(Startup, scene.spawn())  // 0.20 的新写法，见 §4
    .run();
```

**Schedule 地图**（先记这四个，够用很久）：

| Schedule | Unity 对应 | 时机 |
|---|---|---|
| `Startup` | `Awake/Start` | 启动时一次 |
| `Update` | `Update` | 每帧一次（可变帧率） |
| `RunFixedMainLoop`（内部 `FixedUpdate`） | `FixedUpdate` | 固定步进，Stage 3 细讲 |
| `StateTransition` + 各 `OnEnter/OnExit` | 状态机回调 | Stage 4 细讲 |

## 3. 组件（Component）：数据的乐高

```rust
// marker 组件：不装数据，只贴标签（≈ "这个实体是玩家"）
#[derive(Component)]
struct Player;

// 数据组件
#[derive(Component)]
struct Health { current: f32, max: f32 }
```

组件就是普通 struct + `#[derive(Component)]`。**可以带逻辑方法**（`impl Health { fn is_dead(&self) }`），但组件永远不会像 MonoBehaviour 那样自己被"调用"——它只被 system 查询。

## 4. 生成实体：0.20 的两种写法

### 写法 A：经典 Commands（你练习里主用这个）

摘自 `examples/3d/3d_shapes.rs`：

```rust
fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 地面
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(50.0, 50.0))),
        MeshMaterial3d(materials.add(Color::from(SILVER))),
    ));

    // 立方体 + 自定义组件
    commands.spawn((
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(debug_material.clone()),
        Transform::from_xyz(0.0, 0.5, 0.0),
        Shape,                       // ← marker 组件直接混进元组
    ));

    // 灯：组件支持 struct 字面量语法 + ..default()
    commands.spawn((
        PointLight {
            shadow_maps_enabled: true,
            intensity: 10_000_000.,
            ..default()
        },
        Transform::from_xyz(8.0, 16.0, 8.0),
    ));

    // 相机
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 7.0, 14.0).looking_at(Vec3::new(0., 1., 0.), Vec3::Y),
    ));
}
```

要点：

- `spawn((...))` 的元组就是"组件包"（老版本的 Bundle 类型基本隐身了，元组直接用）
- mesh/材质**不是组件**，是**资产**（`Assets<T>` 仓库里的 T），实体上挂的是 `Mesh3d(Handle)` / `MeshMaterial3d::<StandardMaterial>(Handle)`。
  为什么套一层？官方注释原话：Handle 本身不实现 Component，**newtype 防歧义**（同一个 Handle 可能用于不同目的）。这个设计以后在资产阶段会反复遇到
- `materials.add(Color::from(SILVER))`：`add` 收「任何能变成 T 的东西」（Color → StandardMaterial 有 From），比老版本 `StandardMaterial::from(...)` 更省
- `Transform::from_xyz(x, y, z)` / `.looking_at(目标, 上方向)`：和 Unity 惯用法几乎一致，注意 **Y-up、右手系**（和 Unity 一样，幸福感 +1）
- `Commands` 是**延迟执行**的：本轮排队，下一个 sync point（调度点）才生效。想"立刻看到刚 spawn 的实体"请读 `ecs/delayed_commands`

### 写法 B：BSN（0.20 新场景语法，先认识即可）

摘自 `examples/3d/3d_scene.rs`：

```rust
fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_systems(Startup, scene.spawn())
        .run();
}

fn scene() -> impl SceneList {
    bsn_list! {
        #CircularBase                        // ← 静态字符串当标记
        Mesh3d(asset_value(Circle::new(4.0)))
        MeshMaterial3d::<StandardMaterial>(asset_value(Color::WHITE))
        Transform::from_rotation(Quat::from_rotation_x(-std::f32::consts::FRAC_PI_2))
        --                                    // -- 分隔下一个实体
        #Cube
        Mesh3d(asset_value(Cuboid::new(1.0, 1.0, 1.0)))
        MeshMaterial3d::<StandardMaterial>(asset_value(Color::srgb_u8(124, 144, 255)))
        Transform::from_xyz(0.0, 0.5, 0.0)
        --
        PointLight { shadow_maps_enabled: true }
        Transform::from_xyz(4.0, 8.0, 4.0)
        --
        Camera3d
        Transform::from_xyz(-2.5, 4.5, 9.0).looking_at(Vec3::ZERO, Vec3::Y)
    }
}
```

声明式写场景，`#名字` 直接做标记组件，`asset_value` 包资产。**现阶段读懂即可，练习用写法 A**——Commands 是理解 ECS 的正路。

## 5. Query：按数据形状取数据

```rust
fn rotate(time: Res<Time>, mut cubes: Query<&mut Transform, With<Rotator>>) {
    for mut tr in &mut cubes {
        tr.rotate_y(time.delta_secs() * 0.5);
    }
}
```

Query 泛型参数 = 「我要什么数据」：

| 写法 | 含义 |
|---|---|
| `Query<&Transform>` | 只读引用（可很多 system 并行共享） |
| `Query<&mut Transform>` | 可写引用（互斥，调度器会自动避免冲突并行） |
| `Query<(&Health, &mut Transform)>` | 组合多组件（实体必须**同时**有） |
| `Query<&Transform, With<Rotator>>` | 过滤器：只要有 Rotator 的（但**不取** Rotator 数据） |
| `Query<&Transform, Without<Camera3d>>` | 排除某类 |
| `Query<Option<&Children>>` | 组件可能没有：`Option` 包裹 |
| `Query<Entity, With<Enemy>>` | 只要实体 ID |

常用方法：

```rust
q.iter() / for x in &q          // 遍历
q.iter_mut() / for x in &mut q  // 可变遍历
q.single().unwrap()             // 「有且仅有一个」（0.20 返回 Result！）
q.get(entity).unwrap()          // 按 Entity ID 取
q.iter().len() / is_empty()
```

> 为什么参数是这些？**system 的参数列表就是它的"访问声明"**，调度器据此决定谁能和谁并行。这就是 primer 里说的借用检查的运行时版，也是 Bevy 免费多线程的根。

## 6. Resource：全局单例

```rust
#[derive(Resource, Default)]
struct GameConfig { speed: f32, player_hp: f32 }

// 注册（三种来源）：
App::new()
    .init_resource::<GameConfig>()          // 要求 Default
    .insert_resource(GameConfig { speed: 2.0, player_hp: 100.0 })   // 直接给值
    .add_systems(Update, use_config);

// 读取 / 写入：
fn use_config(time: Res<Time>, cfg: Res<GameConfig>, mut w: ResMut<GameConfig>) { ... }
```

`Res<T>` 只读、`ResMut<T>` 可写——与 query 的借用规则一致。凡是"跨 system 共享的一份数据"（配置、计时器、输入状态、渲染设置）都是 Resource。`Res<ButtonInput<KeyCode>>` 就是引擎内置的键盘输入资源。

还有 `Local<T>`：system 私有的持久状态（每个 system 一份，别人看不见）。

## 7. 事件：Message 与 Observer（0.20 重点，旧教程全错的地方）

0.17 起 Bevy 把「跨 system 消息」从 Event 改名为 **Message**。你搜到的 `EventWriter` 教程都是旧版。

### Message：队列，一发多收

摘自 `examples/ecs/message.rs`：

```rust
// 定义（和 Component/Resource 并列的三大 derive 之一）
#[derive(Message, Debug)]
struct DealDamage { amount: i32 }

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_message::<DealDamage>()        // ← 必须注册，忘了就是运行时 panic
        .add_systems(Update, (deal_damage, apply_damage).chain())
        .run();
}

fn deal_damage(mut writer: MessageWriter<DealDamage>) {
    writer.write(DealDamage { amount: 10 });
}

fn apply_damage(mut reader: MessageReader<DealDamage>) {
    for dmg in reader.read() {
        info!("Applying {} damage", dmg.amount);
    }
}
```

- `MessageWriter<T>` 发、`MessageReader<T>` 收（每个 reader 独立游标，多个 system 都能收到同一条）
- `MessageMutator<T>` = 读写一体（少见，message.rs 里有演示）
- 消息**两帧后清空**，reader 必须每帧读才不漏
- **典型用途**：输入→逻辑、逻辑→音频/特效/UI 的解耦通信

### Observer：触发式回调，事件+目标实体

摘自 `examples/ecs/observers.rs`：

```rust
// 自定义触发事件
#[derive(Event)]
struct Explode { entity: Entity }

App::new()
    // 监听自定义事件（闭包或函数都行，还能挂 run_if！）
    .add_observer(|explode: On<Explode>, mines: Query<&Mine>, mut commands: Commands| {
        for e in nearby(explode.pos) {
            commands.trigger(Explode { entity: e });   // 观察者里再触发，形成级联
        }
    })
    // 监听组件生命周期
    .add_observer(on_add_mine)
    .run();

fn on_add_mine(add: On<Add<Mine>>, query: Query<&Mine>, mut index: ResMut<SpatialIndex>) { ... }
fn on_remove_mine(remove: On<Remove<Mine>>, ...) { ... }   // despawn 也会触发 Remove
```

- `On<Explode>`：自定义触发；`On<Add<T>>` / `On<Remove<T>>` / `On<Insert<T>>` / `On<Replace<T>>`：生命周期钩子（0.20 生命周期签名是 `On<Add<T>>` 这种泛型形式）
- `commands.trigger(...)` 触发、还能 `trigger_observers` / observers.rs 里有传播（`observer_propagation.rs`）
- **典型用途**：对"某个实体发生的事"做反应（出生初始化、死亡清理、级联爆炸）；对引擎内置事件做响应

### 怎么选

| | Message | Observer |
|---|---|---|
| 模型 | 帧内队列，广播 | 立即/定向回调 |
| 适合 | 系统 A 通知系统 B（松耦合） | 对特定实体/事件的响应逻辑 |
| 时机 | 依赖 system 排序 | trigger 处执行 |

## 8. System 排序与调度

**不排序时 systems 顺序不确定**（可并行，靠参数冲突避免数据竞争，但先后没保证）。需要顺序时：

```rust
// 1. chain：按序执行
.add_systems(Update, (sys_a, sys_b, sys_c).chain())

// 2. before/after：只约束相对关系
.add_systems(Update, (sys_a, sys_b.after(sys_a)))

// 3. run_if：条件执行（也接受函数/内建条件）
.add_systems(Update, toggle_wireframe.run_if(input_just_pressed(KeyCode::Space)))
```

`ecs/message.rs` 的注释特意强调：**Writer 必须排在 Reader 前面**（chain），否则 Reader 本帧读到的是上一帧残留或空。这是新手消息"时灵时不灵"的头号原因。

排查顺序问题的神器：`ecs/system_stepping.rs`（逐帧单步执行 systems）和 `showcase/stepping.rs`。

## 9. 带 proton 的问题读 example（阅读任务）

按顺序读，每篇带一个问题：

1. `hello_world` → 最小骨架长什么样？
2. `3d/3d_shapes.rs` → **精读 setup**：mesh/material handle 怎么来的？marker 组件怎么用？两种灯/相机参数什么含义？（改：把地面 50x50 改 10x10、换形状数组，跑起来看）
3. `ecs/ecs_guide.rs` → 全文精读（注释即教程），重点：query 组合/Option/With/Without、Bundles 与组件元组、Change Detection
4. `ecs/message.rs` → 五个 system 的链路为什么必须 chain？
5. `ecs/observers.rs` → `On<Add>` 和普通 startup system 干初始化有什么区别？级联爆炸怎么实现的？
6. `ecs/run_conditions.rs`、`ecs/delayed_commands.rs`、`app/plugin.rs`（把一组功能打包成 Plugin 的正确姿势——你的练习代码将从单文件逐步模块化，Stage 2 开始用）
7. `time/timers.rs` → Timer/Stopwatch，动画/技能 CD 的基础

## 10. 本阶段练习

打开 `practice/p01_first_scene/src/main.rs`，按注释里的 `TODO` 顺序完成（每完成一个 `cargo run` 验证一次）。任务依次用到：marker 查询 → Resource 调参 → Message → Observer → despawn。做完本阶段毕业。
