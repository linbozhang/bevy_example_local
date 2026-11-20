# Bevy 系统学习路径（Bevy 0.20 · Mac + Zed/Helix）

> 定制条件：Rust「能读不能写」→ 配 Unity C# 对照速查；**玩法 + 渲染双修**；**3D 起步**（TA 背景对口）。

## 学习哲学

**以官方 examples 为主干，中文导读 + 独立练习为两翼。**

- Bevy 每 3~4 个月破坏性升级，任何手写教程都会过时；官方 examples 是唯一永远与 API 同步的教材
- 但官方 examples 按类别组织（200+ 个），没有学习顺序。本仓库把它们重排成 9 个阶段
- **版本锚点**：本机 `bevy-fork-pinned`（Bevy **0.20.0** 源码），练习用 crates.io 的 **bevy 0.20.1**（同 minor，API 一致）。导读中引用的所有 example 路径都来自这份 fork，可直接跳源码
- ⚠️ 网上教程/视频（含 AI 训练数据里的旧写法）大量停留在 0.13~0.16：那时事件叫 `Event/EventWriter`（现在是 `Message/MessageWriter`）、相机是 `Camera3dBundle`（现在是 `Camera3d` + 组件元组）、`Query::single()` 直接返回引用（现在返回 `Result`）。**遇到教程和 example 冲突，一律以 example 为准**

### 每个知识点的学习循环

1. **跑**：用 `run_example.sh` 跑对应官方 example，先看现象
2. **读**：对照 notes/ 导读读 example 源码，弄懂每一段
3. **改**：改 example 参数/逻辑重跑（在 fork 里改完 `git checkout` 还原）
4. **练**：完成 practice/ 对应练习（独立 crate，不抄 example）

---

## 环境篇

### 已确认的环境

| 项 | 状态 |
|---|---|
| Rust | ✅ rustc 1.99.1 (stable, rustup 管理) |
| Bevy 源码 fork | ✅ `~/workspace/github/bevy-fork-pinned`（0.20.0） |
| 练习依赖 | crates.io `bevy = "0.20.1"`（workspace 统一管理） |

### Zed

仓库已带 `.zed/settings.json`（rust-analyzer + 保存时格式化 + 自动 fix）。用 Zed 打开本仓库根目录即可。常用操作：

- `cmd-shift-p` → "rust-analyzer: Restart Server"（改依赖后如代码提示异常时用）
- hover 看 API 文档、`F2` 重命名、`cmd-.` 代码操作（自动导入 import）

### Helix

Helix 通过 rustup 自动找到 rust-analyzer（若 LSP 未启动：`rustup component add rust-analyzer`）。本项目直接 `hx` 打开即可。常用：

- `Space + h` 跳到定义 / `Space + r` 重命名 / `Space + f` 查找符号 / `gd` 跳定义

### 编译加速（重要）

Bevy 全量编译很重。已做两件事：

1. **workspace 根 Cargo.toml 已配官方推荐 dev profile**（自身逻辑 O1 + 依赖 O3）
2. **dynamic_linking**（增量编译从 ~40s 降到 ~2s）：

   ```bash
   cargo run -p p01_first_scene --features dynamic_linking
   ```

   ⚠️ 开与不开该 feature 的编译产物**不能混用**，切换时报错就 `cargo clean -p p01_first_scene`。日常开发建议始终带上；最终验证性能时用 `--release` 跑一次（不带 dynamic_linking）。

3. 首次编译任何 crate 都要拉取并编译 ~600 个依赖（几分钟量级，属正常）

### 跑官方 example

```bash
./run_example.sh 3d/3d_scene        # 大部分 example 直接跑
./run_example.sh ecs/ecs_guide      # 不带 .rs 后缀，子目录用 /
```

（等价于在 fork 目录里 `cargo run --example 3d/3d_scene`。首次会编译 fork 里的 bevy 本体，较久。）

---

## 目录结构

```
bevy_example_local/
├── README.md            ← 你在这里（课程总图 + 进度表）
├── Cargo.toml           # cargo workspace（practice 统一管理）
├── run_example.sh       # 一键跑 fork 里的官方 example
├── notes/               # 每阶段中文导读
│   ├── 00-rust-primer.md     # Rust 速查（Unity C# 对照）
│   └── 01-app-ecs-core.md    # Stage 1 导读
└── practice/            # 练习 crate（每阶段一个）
    └── p01_first_scene/
```

新建后续练习的模板：

```bash
cargo new practice/p02_gltf_scene
# 然后在 practice/p02_gltf_scene/Cargo.toml 里写：
#   [dependencies]
#   bevy.workspace = true
#   [features]
#   dynamic_linking = ["bevy/dynamic_linking"]
# 并把目录加入根 Cargo.toml 的 members
```

---

## 课程总图（9 阶段）

> 所有 example 路径相对 fork 的 `examples/` 目录。⭐ = 必读精读，其余为选读。
> 每阶段「完成标准」达成后再进下一阶段。

### Stage 0 · 环境与 Rust 速查（0.5 天）

| 项 | 内容 |
|---|---|
| 目标 | 跑通环境；建立 Rust→C# 心智映射；不求记住，做到「报错能查」 |
| 阅读 | ⭐ [notes/00-rust-primer.md](notes/00-rust-primer.md) |
| example | `hello_world`（就为跑通） |
| 练习 | 无，环境即练习 |
| 完成标准 | `cargo run -p p01_first_scene` 能出画面；primer 里的对照表过了一遍 |

### Stage 1 · App 与 ECS 内核（3~5 天）★ 最重要的一站

| 项 | 内容 |
|---|---|
| 目标 | system / component / query / resource / commands / message / observer / 排序与调度，全链路打通 |
| 阅读 | ⭐ [notes/01-app-ecs-core.md](notes/01-app-ecs-core.md) |
| example | ⭐ `hello_world`、⭐ `ecs/ecs_guide`（圣经，长文精读）、⭐ `ecs/message`、⭐ `ecs/observers`、⭐ `3d/3d_scene` + `3d/3d_shapes`（两种 spawn 写法）、`ecs/startup_system`、`ecs/run_conditions`、`ecs/delayed_commands`、`ecs/change_detection`、`app/plugin`、`time/timers` |
| 练习 | `practice/p01_first_scene`（TODO 清单在源码注释里）；[p02_rpg_demo](practice/p02_rpg_demo/README.md)（无窗口 RPG 战斗：消息流水线、AOE、同时结算、Boss 连锁死亡） |
| 完成标准 | 不看资料能独立写出：marker 组件过滤查询 + Resource 调参 + Message 通信 + Observer 监听组件添加；能说清 Message 与 Observer 的取舍 |

### Stage 2 · 3D 场景与资产（3~4 天）

| 项 | 内容 |
|---|---|
| 目标 | 层级、灯光、gltf 加载（含 skinned mesh）、资产系统初步、相机控制 |
| example | ⭐ `gltf/load_gltf`、⭐ `3d/parenting`、⭐ `camera/camera_orbit`、`3d/lighting`、`gltf/gltf_skinned_mesh`、`asset/asset_loading`、`transforms/transform`（若有）、`camera/projection_zoom` |
| 练习 | `p02_gltf_scene`：加载 glb → orbit 相机环绕 → 层级挂点（把一个物体挂到骨骼/父节点下） |
| 完成标准 | glb 模型 + 骨骼动画播放出来，相机能绕目标旋转缩放；理解 `Assets<T>` / `Handle` 与 `asset_value` 的关系 |

### Stage 3 · 输入、时间与移动手感（2~3 天）

| 项 | 内容 |
|---|---|
| 目标 | 键鼠/手柄输入、固定步进物理、插值、Time 体系 |
| example | ⭐ `movement/physics_in_fixed_timestep`、⭐ `camera/free_camera_controller`、`input/keyboard_input`、`input/mouse_input`、`input/gamepad_input`、`time/time`、`time/virtual_time`、`movement/smooth_follow` |
| 练习 | `p03_controller`：WASD + 鼠标视角的第一/第三人称控制器，跳跃用固定步进积分，相机平滑跟随 |
| 完成标准 | 能解释 `Update` 与 `FixedUpdate`（RunFixedMainLoop）的执行时机差异，以及为什么物理要放固定步进 |

### Stage 4 · 状态、UI、音频、动画（4~5 天）

| 项 | 内容 |
|---|---|
| 目标 | 状态机切流程、UI 布局与样式、音频、动画系统（AnimationGraph） |
| example | ⭐ `state/states`、⭐ `showcase/game_menu`（状态+UI 综合最佳示例）、`state/sub_states`、`state/computed_states`、`ui/text/`（text_fonts 等）、`ui/styling/`、`ui/widgets/`、`audio/audio`、`audio/soundtrack`、`audio/spatial_audio_3d`、`animation/animated_mesh`、`animation/animation_graph` |
| 练习 | `p04_game_shell`：主菜单 → 游戏中 → 暂停 → 结算 的完整状态流 + HUD（分数/血条）+ BGM 切换 |
| 完成标准 | 状态切换带 OnEnter/OnExit 处理（清理实体、切 UI），UI 能响应游戏数据 |

### Stage 5 · 中期综合项目（1~2 周）★ 玩法线收官

| 项 | 内容 |
|---|---|
| 目标 | 综合 Stage 1~4 做一个**小而完整**的 3D 小游戏（范围克制！） |
| 参考 | `showcase/mines`、`showcase/alien_cake_addict`、`showcase/desk_toy`、`showcase/breakout`（2D 但结构好）、`showcase/loading_screen` |
| 练习 | `p05_mini_game`：建议「3D 收集/躲避」或简化塔防——固定步进移动 + 碰撞(手写距离判定即可) + 计分 UI + 状态流 + 音效 |
| 完成标准 | 别人拿到能玩懂；代码按 system 职责拆分（不再是一个大文件）；有至少一个 Message 链路和一个 Observer 链路 |

> ⚠️ 到这里玩法线毕业。后面进入渲染/TA 主场，节奏可放慢，按工作需要选修。

### Stage 6 · 渲染/TA 专场 I · 材质与 Shader（1 周）

| 项 | 内容 |
|---|---|
| 目标 | 自定义 Material（AsBindGroup + 继承 StandardMaterial）、wgsl shader、shader_defs 变体、GPU 资源读写 |
| example | ⭐ `shader/shader_material`、⭐ `shader/extended_material`、⭐ `shader/animate_shader`、`shader/shader_defs`、`shader/gpu_readback`、`shader/storage_buffer`、`shader/pipeline_constants`、`shader/fallback_image` |
| 练习 | `p06_custom_material`：写一个带顶点波动 + 时间驱动的自定义水面/旗帜材质；再写一个 `ExtendedMaterial`（StandardMaterial + 描边/emissive 脉冲） |
| 完成标准 | 能说清 Material trait → shader 绑定 → 渲染管线的对应关系（类比 URP 的 ShaderGraph 输出到 SRP Batcher 的映射）；会开 shader_defs 条件编译 |

### Stage 7 · 渲染/TA 专场 II · 后处理与 Render Graph（1~2 周）★ TA 核心

| 项 | 内容 |
|---|---|
| 目标 | 全屏后处理 pass、自定义 render node/phase、compute shader——对标 URP ScriptableRenderPass/Renderer Feature 的知识 |
| example | ⭐ `shader_advanced/custom_post_processing`、⭐ `3d/post_processing`（Bloom/TONemapping/DOF 等官方 Pass 如何注册）、⭐ `shader/compute_shader_game_of_life`、`shader_advanced/custom_phase_item`、`shader_advanced/fullscreen_material`、`shader_advanced/render_depth_to_texture`、`3d/render_to_texture` |
| 练习 | `p07_post_process`：自定义全屏 pass（如屏幕空间轮廓/径向模糊/LUT 校色三选一），带强度参数运行时可调 |
| 完成标准 | 能画出 Extract→Prepare→Queue→Phase sort→Render 的数据流图，解释 view/target 生命周期；compute shader 能读写 storage buffer |

### Stage 8 · 渲染/TA 专场 III · 专题选修（持续）

按兴趣/工作需要选修，每个都是一顿大餐：

- **光照/阴影**：`3d/pcss`、`3d/contact_shadows`、`3d/shadow_biases`、`3d/mixed_lighting`、`3d/light_textures`、`3d/rect_light`、`3d/spherical_area_lights`
- **大气/雾**：`3d/volumetric_fog`、`3d/fog_volumes`、`3d/atmosphere`、`3d/atmospheric_fog`、`3d/scrolling_fog`
- **屏幕空间**：`3d/ssao`、`3d/ssr`、`3d/auto_exposure`、`3d/color_grading`、`3d/tonemapping`、`3d/bloom_3d`、`3d/motion_blur`、`3d/depth_of_field`
- **高级管线**：`3d/deferred_rendering`、`3d/order_independent_transparency`、`3d/meshlet`、`3d/occlusion_culling`、`3d/solari`（GI）、`3d/irradiance_volumes`、`3d/reflection_probes`
- **mesh/材质底层**：`shader_advanced/specialized_mesh_pipeline`、`shader_advanced/manual_material`、`shader_advanced/mesh_shader_intro`（mesh shader!）、`shader_advanced/texture_binding_array`（bindless）、`3d/generate_custom_mesh`
- **性能诊断**：`diagnostics/`（enabling diagnostics 等）、`dev_tools/`

练习：`p08_tech_demo` —— 自选主题做技术验证 demo（推荐：卡通渲染 = cel-shading 材质 + 屏幕空间 outline 后处理，正好串联 Stage 6+7）

### Stage 9 · 引擎深入（选修，无终止）

| 项 | 内容 |
|---|---|
| 目标 | 调度器内部、自定义 schedule/executor、headless 渲染、reflection、大场景优化 |
| example | `ecs/custom_executor`、`ecs/custom_schedule`、`ecs/system_param`（自定义查询参数）、`ecs/generic_system`、`app/headless_renderer`、`app/externally_driven_headless_renderer`、`reflection/reflection`、`large_scenes/`（many_cubes 等）、`stress_tests/bevymark` |
| 方式 | 这个阶段 example 只是入口，主体是**读 fork 源码**：`crates/bevy_ecm`（调度）、`crates/bevy_render`（渲染图）等——和你读 napengine 源码同一套打法 |

---

## 进度表

- [ ] Stage 0 · 环境与 Rust 速查
- [ ] Stage 1 · App 与 ECS 内核
- [ ] Stage 2 · 3D 场景与资产
- [ ] Stage 3 · 输入、时间与移动手感
- [ ] Stage 4 · 状态、UI、音频、动画
- [ ] Stage 5 · 中期综合项目（玩法线毕业）
- [ ] Stage 6 · 渲染 I · 材质与 Shader
- [ ] Stage 7 · 渲染 II · 后处理与 Render Graph
- [ ] Stage 8 · 渲染 III · 专题选修
- [ ] Stage 9 · 引擎深入

## 卡住时去哪

1. 本仓库 notes/ 对应阶段导读
2. fork 里 `examples/` 找最接近的官方 example（这是最可靠的答案来源）
3. API 文档：`cargo doc -p bevy --open`（本地生成，版本保证一致）
4. [Bevy 官方讨论/Discord](https://bevyengine.org/community/) 与 [GitHub discussions](https://github.com/bevyengine/bevy/discussions)
