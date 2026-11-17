# 00 · Rust 速查（写给 Unity C# 背景的你）

> 定位：你**能读** Rust，问题是**写不出来**。本篇不是 Rust 教程，是「把读得懂变成写得出」的最小肌肉记忆集 + Bevy 场景下最高频的坑。
> 用法：通读一遍 → 练习时随用随查。所有语法都在 Bevy 代码语境下讲。

## 0. 心智模型：四个转换

| C# 习惯 | Rust 现实 | 写代码时的直觉 |
|---|---|---|
| GC 管对象，引用随便传 | **所有权**：每个值只有一个主人，传出去就没了 | 传引用要显式 `&`/`&mut`；想复制打 `.clone()` |
| `class` 一把梭 | **struct + trait** 组合：数据和行为分离 | 数据放 struct 字段，行为用 `impl`/trait 提供 |
| `null` / `Nullable` | **没有 null**，用 `Option<T>` | 「可能没有」在类型里写出来，编译器逼你处理 |
| `try/catch` 异常 | **没有异常**，用 `Result<T, E>` + `?` | 函数签名会「说谎藏错误」？不存在的，错误都在类型里 |

另外两条贯穿一切：

- **默认不可变**。要改就 `let mut x` / `&mut`。Bevy 里 `Res<T>`（只读）vs `ResMut<T>`（可写）就是这个思想
- **表达式优先**。`if`/`match`/块都有值：`let x = if a { 1 } else { 2 };` 函数最后一行不写分号就是返回值

## 1. 变量与类型

```rust
let a = 5;              // 推断 i32，不可变
let mut b = 3.0f32;     // f32，可变；数字后缀强制类型
b += 1.0;
let a = a as f64;       // shadowing：同名重新绑定（不是改值），常用于类型转换
const SPEED: f32 = 4.5; // 常量必须标类型

let v = Vec3::new(1.0, 2.0, 3.0);   // Bevy 数学类型，无 new，全是关联函数
let q = Quat::from_rotation_y(f32::to_radians(90.0));
```

要点：
- **没有隐式数值转换**。`f32` 和 `f64` 混算、`usize` 当 `f32` 用都会编译错，要显式 `as f32`
- 数值字面量可读性下划线：`10_000_000.`
- `i as f32` / `f as i32` / `u as usize` 是你日常显式转换三件套

## 2. 所有权与借用（Bevy 卡人第一名）

三条规则，背下来：

1. 每个值有**唯一拥有者**（变量/字段/结构体）
2. 把值**赋给别的变量 / 传进函数 / 从函数返回** → 所有权**移动**（move），原变量作废
3. 值离开作用域被释放（编译期决定，无 GC）

```rust
let s1 = String::from("hi");
let s2 = s1;            // move！之后用 s1 直接编译错
let s3 = s2.clone();    // 想复制就显式 clone
```

借用 = 临时借来看（不拿走所有权）：

```rust
fn len(s: &String) -> usize { s.len() }      // 共享借用（只读），可同时多个
fn push(s: &mut String) { s.push('!'); }     // 独占借用（可写），同一时刻只能有一个
```

**核心铁律：一个可变借用 XOR 任意多个只读借用**。这就是你 90% 的编译报错来源。

### 在 Bevy 里它长什么样

```rust
fn rotate(time: Res<Time>, mut q: Query<&mut Transform, With<Rotator>>) {
    for mut tr in &mut q {
        tr.rotate_y(time.delta_secs() * 0.5);
    }
}
```

- `Res<Time>`：全局只读借用 → 所以同一帧里其他 system 也能拿 `Res<Time>`，可并行
- `Query<&mut Transform>`：可写借用 → 引擎调度器保证**不会有两个可写相同组件的 system 同帧并行**，这就是 Bevy 并行的原理：靠借用检查的运行时版
- `for mut tr in &mut q`：注意模式里的 `mut`——迭代给的是 `Mut<Transform>` 智能指针，`mut tr` 才能调可变方法

**什么时候 `.clone()`**：Bevy 的 `Handle<T>`（资产句柄）clone 只是复制一个整数 ID，**便宜，随便 clone**。大对象（Mesh/Image 数据）才贵。

## 3. struct / enum / 模式匹配

```rust
// 数据体：C# 的 class 字段部分
#[derive(Component)]                 // ← derive 宏：自动实现 Component trait
struct Rotator { speed: f32 }

// 元组结构体：Bevy 大量使用（newtype 包装）
struct DamageTimer(pub Timer);       // 访问：t.0

// 无字段 unit struct：当 marker/tag 用
#[derive(Component)]
struct Player;

// enum：比 C# 强，是「带数据的联合体」+ 完整模式匹配
enum GameState { Playing { score: u32 }, Paused, Over }

match state {
    GameState::Playing { score } if score > 100 => { /* 守卫条件 */ }
    GameState::Playing { .. } => {}
    GameState::Paused | GameState::Over => {}    // 多模式合并
}
```

`match` 必须**穷尽**所有分支——编译器强制你处理每一种情况（这就是没有 null 异常的原因）。

## 4. trait（≈ interface，但更强）

```rust
trait Despawnable { fn despawn(&self) -> bool; }

impl Despawnable for Mine {
    fn despawn(&self) -> bool { self.fuse <= 0.0 }
}

// 泛型约束：T: Despawnable（≈ where T : IDespawnable）
fn cleanup<T: Despawnable>(items: Vec<T>) { /* ... */ }
```

你主要**使用** trait 而不是定义：`Component`、`Resource`、`Message`、`Plugin`、`Default`、`Clone`……都是给类型贴标签，让引擎认识它。

### derive = 自动 boilerplate

```rust
#[derive(Component, Resource, Clone, Debug, Default)]
struct Speed(f32);
```

Debug→可打印（`{:?}`），Default→`Speed::default()` / `..default()` 语法糖，Clone→可复制。**Bevy 组件建议默认带上 `Component + Debug`**。

## 5. Option / Result / ? 

```rust
// Option<T> = C# 的 T?，但没有 null
let maybe: Option<f32> = query.iter().next().map(|e| e.0);
match maybe { Some(v) => {}, None => {} }
if let Some(v) = maybe { /* 只要 Some 分支 */ }
let v = maybe.unwrap_or(0.0);          // 带默认值解包
maybe.unwrap();                         // None 就 panic —— Bevy 练习里可以先用，改 TODO 时再换成优雅写法

// Result<T, E>：可恢复错误
fn load() -> Result<Asset, std::io::Error> { /* ... */ }
let a = load()?;                        // ? = 出错提前 return Err，成功解包。只能在返回 Result 的函数里用
```

Bevy 0.20 注意：`query.single()` 返回 `Result<...>`，惯用写法：

```rust
let cam = camera_q.single().unwrap();   // example 里最常见的偷懒写法
// 严谨写法：match / map_err / .expect("scene must have exactly one camera")
```

## 6. 闭包与迭代器

```rust
let nums = vec![1, 2, 3];
let doubled: Vec<i32> = nums.iter().map(|n| n * 2).collect();   // [2,4,6]
let evens: Vec<&i32> = nums.iter().filter(|n| **n % 2 == 0).collect();

// 排序/累积/查找——写游戏逻辑的日常
entities.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
let total: f32 = damages.iter().sum();
let target = enemies.iter().find(|e| e.1.hp < 30.0);
```

- 闭包：`|参数| 表达式`；捕获环境变量自动推断（move 语义：`move |x| ...` 强制拿走所有权，Bevy 线程池里常见）
- 迭代器是**惰性**的，不 `collect`/`for`/`sum` 消费就不执行
- `iter()` 借用 / `into_iter()` 拿走所有权 / `iter_mut()` 可变借用——三兄弟对应 &/&move/&mut

## 7. 宏：三类，认脸即可

```rust
#[derive(Component)]      // 1. derive：贴标签自动生成实现（最常用）
println! / info!          // 2. 声明式宏：语法糖（带 ! 调用）
bsn_list! { ... }         // 3. 过程宏：编译期代码生成（Bevy 的场景 DSL）
```

你不需要会写宏，需要认得「带 `!` 的调用是宏，参数格式可以很怪」。

## 8. 模块系统（vs namespace）

```rust
// 文件即模块：src/enemies.rs 里的内容默认私有
mod enemies;              // main.rs 声明：我要这个子模块
use enemies::Mine;        // 导入路径（≈ using）

pub struct Mine;          // pub 才能被外部用；pub(crate) 限本 crate

// Bevy 实战里最常用的一句：
use bevy::prelude::*;     // 把常用 95% 的类型全倒进来，example 全这么写，你也这么写
```

## 9. 常见编译报错速查（Bevy 场景）

| 报错关键词 | 真实原因 | 常见修法 |
|---|---|---|
| `E0382 borrow of moved value` | 值被 move 走了还在用 | `.clone()`；或用 `&` 借用而不是传值 |
| `cannot borrow as mutable` / `second mutable borrow` | 同时可变借用 + 又读又写 | 拆作用域：先 `let x = q.get_mut(e)` 用完释放再读；或把数据 clone 出来改完写回 |
| `E0277 the trait bound ... is not satisfied` | 类型没贴对标签（忘了 `#[derive(Component)]`、资源没 `init_resource`、Message 没 `add_message`） | 看报错里**缺哪个 trait**，多半是漏 derive 或漏注册 |
| `mismatched types` | 十有八九是 `f32`/`f64`、`usize`/`i32` 混用，或忘了把 `Duration` 换秒 | `as f32`；`time.delta_secs()` 而不是 `time.delta()` |
| `expected struct X, found (&X,)` 等 tuple 错 | spawn 的组件写成了 `(single,)` 或反了括号 | spawn 永远是 `commands.spawn((组件A, 组件B, ...))` |
| `cannot find type X in this scope` | 忘 `use`；或新旧版本类型名不对（如旧教程的 `EventWriter`） | `use bevy::prelude::*;`；对照 fork example 的 import 抄 |
| `unused variable` 警告 | 占位参数没用到 | 参数名前加下划线：`_unused`（写练习骨架时的标准做法） |

## 10. C# → Rust 对照小抄

| C# | Rust |
|---|---|
| `public class Foo { }` | `struct Foo { }`（+ trait 提供行为） |
| `List<T>` / `Dictionary<K,V>` | `Vec<T>` / `HashMap<K,V>`（Bevy 里用 `bevy::platform::collections::HashMap`） |
| `foreach (var x in list)` | `for x in &list { }` |
| `x?.y ?? z` | `opt.map(|x| x.y).unwrap_or(z)` |
| `Action<T>` 事件 | `Message`（跨 system 队列）/ `Observer`（触发式响应） |
| `ToString()` | `format!("{:?}", x)`（Debug）/ `Display` 的 `{}` |
| `$"hp: {hp}"` | `format!("hp: {hp}")` |
| LINQ `Select/Where` | `.iter().map(...).filter(...)` |
| `Mathf.Lerp` | `f32::lerp(a, b, t)`（Bevy 的 `bevy_math::ops` / `FloatExt`） |
| `Time.deltaTime` | `time.delta_secs()`（f32 秒） |
| 属性 `get/set` | 直接字段访问；需要计算时写方法 |
| `as` / cast | `as`（数值）或 `Into::into`（trait 转换） |

## 11. 一个完整的「翻译」示例

C# Unity（每帧旋转挂了脚本的物体）：

```csharp
public class Rotator : MonoBehaviour {
    public float speed = 1.0f;
    void Update() { transform.Rotate(Vector3.up, speed * Time.deltaTime); }
}
```

Bevy 0.20：

```rust
use bevy::prelude::*;

#[derive(Component)]
struct Rotator { speed: f32 }

fn rotate(time: Res<Time>, q: Query<(&Rotator, &mut Transform)>) {
    for (rot, mut tr) in &mut q {
        tr.rotate_y(rot.speed * time.delta_secs());
    }
}
// 注册：.add_systems(Update, rotate)
```

看出结构了吗：**MonoBehaviour = Component(数据) + System(逻辑) 拆开**。挂脚本 = 实体上放 `Rotator` 组件；`Update()` = 一个能查到它的全局 system。这就是 ECS 的全部起点。
