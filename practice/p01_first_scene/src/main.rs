//! p01_first_scene —— Stage 1 练习：App / ECS 内核
//!
//! 运行:
//!   cargo run -p p01_first_scene
//!   cargo run -p p01_first_scene --features dynamic_linking   # 快速迭代推荐
//!
//! 导读: notes/01-app-ecs-core.md
//! 任务: 按 TODO 1~4 顺序完成，每完成一个就跑起来验证。
//!      骨架可以直接编译运行（TODO 函数是空壳），画面里有三个方块、地面、灯和 HUD。

use bevy::color::palettes::basic::SILVER;
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        // ---- 资源注册（Resource 必须先注册才能被 system 取用，否则 panic）----
        .init_resource::<RotationSpeed>()
        .init_resource::<CubeRegistry>()
        // ---- 消息注册（Message 必须显式注册；这里已替你注册，TODO 2 写收发两端）----
        .add_message::<SpawnCube>()
        // ---- 观察者 ----
        .add_observer(on_cube_added)
        // ---- 系统 ----
        .add_systems(Startup, setup)
        .add_systems(
            Update,
            (
                rotate_cubes,            // 参考实现（已完成）
                adjust_speed,            // TODO 1
                request_spawn_cube,      // TODO 2a
                spawn_cube_from_message, // TODO 2b
            ),
        )
        .run();
}

// ============ 组件 ============

/// marker 组件：只有带它的立方体才会自转（对照实验：中间那个不带）
#[derive(Component)]
struct Rotator;

/// 业务标记：场景里所有"练习方块"。
/// TODO 3 的观察者会监听它的 Add 生命周期事件。
#[derive(Component, Debug)]
struct Cube;

// ============ 资源 ============

/// 全局自转角速度（rad/s）。
/// `pub f32` 让外部能改；写成元组结构体 `.0` 访问。
#[derive(Resource)]
pub struct RotationSpeed(f32);

impl Default for RotationSpeed {
    fn default() -> Self {
        Self(0.5)
    }
}

/// 已 spawn 方块的实体记录，给 TODO 4（挑战）用
#[derive(Resource, Default)]
#[allow(dead_code)] // TODO 2b/4 会写入和读取它；全部完成后删掉本行
struct CubeRegistry(Vec<Entity>);

// ============ 消息 ============

/// "请求生成一个新方块"。空消息体，用 write_default() 发送即可。
#[derive(Message, Default)]
struct SpawnCube;

// ============ Startup ============

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // 地面（Plane3d 默认朝上，无需转 X 轴 —— 与 Circle 铺地不同）
    commands.spawn((
        Mesh3d(meshes.add(Plane3d::default().mesh().size(20.0, 20.0))),
        MeshMaterial3d(materials.add(Color::from(SILVER))),
    ));

    // 三个立方体：两个带 Rotator（会转），中间的没带（对照）
    for i in 0..2 {
        commands.spawn((
            Cube,
            Rotator,
            Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
            MeshMaterial3d(materials.add(Color::srgb(0.9, 0.35, 0.2))),
            Transform::from_xyz(-4.0 + i as f32 * 8.0, 0.5, 0.0),
        ));
    }
    commands.spawn((
        Cube,
        Mesh3d(meshes.add(Cuboid::new(1.0, 1.0, 1.0))),
        MeshMaterial3d(materials.add(Color::srgb(0.2, 0.5, 0.9))),
        Transform::from_xyz(0.0, 0.5, 0.0),
    ));

    // 灯 + 相机（写法与官方 3d/3d_scene.rs 一致）
    commands.spawn((
        PointLight {
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(4.0, 8.0, 4.0),
    ));
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 6.0, 12.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
    ));

    // HUD 文字（UI 根节点，屏幕左上角）
    commands.spawn((
        Text::new("角速度: 0.50 rad/s  (↑/↓ 调整)"),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            ..default()
        },
    ));

}

// ============ Update：参考实现 ============

/// 已完成：带 Rotator 的方块按全局速度自转。
/// 注意三个参数的"借用声明"：只读时间、只读速度、可写 Transform。
fn rotate_cubes(
    time: Res<Time>,
    speed: Res<RotationSpeed>,
    mut cubes: Query<&mut Transform, With<Rotator>>,
) {
    for mut transform in &mut cubes {
        transform.rotate_y(time.delta_secs() * speed.0);
    }
}

// ============ Update：TODO ============

/// TODO 1: 按 ↑/↓ 调整 RotationSpeed，并把当前值实时显示到 HUD。
///
/// 提示:
///   - keyboard.just_pressed(KeyCode::ArrowUp) → speed.0 += 0.25，ArrowDown 反向
///   - 速度别为负: speed.0 = (speed.0 - 0.25).max(0.0)
///   - 更新文字: text.single_mut().unwrap().0 = format!("角速度: {:.2} rad/s", speed.0);
///     （Text 是元组结构体，String 在 .0 字段；single_mut() 在 0.20 返回 Result）
fn adjust_speed(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut speed: ResMut<RotationSpeed>,
    mut text: Query<&mut Text>,
) {
    let _ = (keyboard, &mut speed, &mut text); // 开始写之前删掉这一行
}

/// TODO 2a: 按空格发送 SpawnCube 消息（writer.write(...) 或 write_default()）。
fn request_spawn_cube(keyboard: Res<ButtonInput<KeyCode>>, mut writer: MessageWriter<SpawnCube>) {
    let _ = (keyboard, &mut writer); // 开始写之前删掉这一行
}

/// TODO 2b: 读取 SpawnCube 消息，每次收到就在随机圈上 spawn 一个新方块。
///
/// 提示:
///   - for _spawner in reader.read() { ... }，一次消息 spawn 一个
///   - spawn 写法参考 setup()；新方块也挂上 Cube + Rotator
///   - 位置避免重叠: 用 CubeRegistry 计数 n，摆成圆环
///     angle = n as f32 * 0.7; (angle.cos() * 6.0, 0.5, angle.sin() * 6.0)
///   - 想让 TODO 3 自动改颜色/位置的话，这里 spawn 完就别管了，交给观察者
fn spawn_cube_from_message(
    mut reader: MessageReader<SpawnCube>,
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    registry: ResMut<CubeRegistry>,
) {
    let _ = (&mut reader, &mut commands, &mut meshes, &mut materials, registry);
    // 开始写之前删掉上面这一行
}

/// TODO 3: 观察者 —— 监听 Cube 组件的 Add 事件，把新方块改成随机 HSL 颜色。
///
/// 提示:
///   - 0.20 拿实体是字段访问: trigger.entity（没有 .target() 方法！）
///   - 换材质要插入新的 MeshMaterial3d，参数里加 ResMut<Assets<StandardMaterial>>
///     materials.add(Color::hsl(hue, 0.8, 0.6)) 可以直接生成彩色材质
///   - 验证: 场景启动时最初三个方块也会触发一次 Add（想想为什么，这对不对？）
fn on_cube_added(trigger: On<Add<Cube>>, mut commands: Commands) {
    let _ = (&trigger, &mut commands); // 开始写之前删掉这一行
    // 示例骨架: commands.entity(trigger.entity) ...
}

// ============ 挑战（选做）============

/// TODO 4（选做）: 按 D 键 despawn 最新生成的方块。完成前先不注册，避免警告。
/// 提示:
///   - CubeRegistry(Vec<Entity>) 记录生成顺序（在哪 push？spawn 的地方或观察者里）
///   - registry.0.pop() 拿到 Option<Entity>，Some(e) → commands.entity(e).despawn()
///   - despawn 是延迟命令，同一帧后续 query 仍能看到实体——想想为什么
///   - 完成后把本函数加入 main 的 add_systems（自己加，练手）

#[allow(dead_code)] // TODO 4 完成并注册后可删掉本行
fn despawn_latest(keyboard: Res<ButtonInput<KeyCode>>, mut commands: Commands, mut registry: ResMut<CubeRegistry>) {
    let _ = (keyboard, &mut commands, &mut registry);
}
