# p03_aibodh_chapter1 — Chapters 1–6：从角色到魔法粒子

在 [第一章的角色](https://aibodh.com/posts/bevy-rust-game-development-chapter-1/) 和 [第二章的五层 WFC 地图](https://aibodh.com/posts/bevy-rust-game-development-chapter-2/) 上，实现 [第三章 Let The Data Flow](https://aibodh.com/posts/bevy-rust-game-development-chapter-3/) 的数据驱动角色系统。使用 Bevy 0.20.1 和 `bevy_procedural_tilemaps = 0.5.0`；默认启动 3D，`--2d` 切到 2D。 两种模式均实现 [第四章 Let There Be Collisions](https://aibodh.com/posts/bevy-rust-game-development-chapter-4/) 的加载、暂停、碰撞和深度排序。

## 运行

在 workspace 根目录：

```sh
cargo run -p p03_aibodh_chapter1        # 3D 方块地图 + 像素角色
cargo run -p p03_aibodh_chapter1 -- --2d # 2D 地图 + 像素角色
cargo test -p p03_aibodh_chapter1
```

也可以进入本目录后运行 `cargo run`。资源目录由 `CARGO_MANIFEST_DIR` 定位，启动时不必复制资源。该路径适合源码练习；若单独分发可执行文件，需要改为可部署的资源路径。

两个模式使用相同的操作：方向键移动，按住任一 `Shift` 奔跑，按 `Space` 跳跃，按 `1`–`6` 原地切换角色。默认角色为 Adventurer。斜向移动归一化；静止时保持最后朝向，显示 Walk 的首帧。

地图为 25×18 格、五层，启动时生成一次。2D 每格 32 像素；3D 每格 1 个世界单位。两种模式都阻挡树干、岩石、树桩、深水和地图边界；浅岸可走。3D 角色依然以陆地表面为高度基准，不做通用物理或战斗。

### 3D 第三人称相机

默认从角色背后跟随。左键单击窗口捕获鼠标，移动鼠标转动视角，`Esc` 暂停并释放鼠标，再按 `Esc` 恢复；恢复后可重新点击捕获。`WASD` 或方向键按相机的水平朝向移动，`Shift` 奔跑，`Space` 跳跃。镜头不随跳跃上下抖动；树木或地面挡在镜头与角色之间时，镜头会拉近。

## 第六章：魔法粒子

参照 [Let There Be Particles](https://aibodh.com/posts/bevy-rust-game-development-chapter-6/)。两个模式都用 `Ctrl` 朝角色当前朝向施放，冷却 0.5 秒；`F5` 火焰、`F6` 奥术、`F7` 暗影、`F8` 毒雾。右上角显示当前法术，原来的 `1`–`6` 仍用于换角。3D 发射方向按相机水平朝向换算，粒子面向相机并受场景深度遮挡。

`magic.rs` 定义四种法术及发射参数；`particles/` 共用发射、运动、旋转、变色、缩小和过期回收逻辑。2D、3D 分别使用各自的 WESL 入口，共用径向辉光函数和加法混合效果。暂停会冻结粒子与冷却；本章只有特效，不实现命中、伤害或法术与地形碰撞。

## 第五章：拾取、背包与镜头跟随

参照 [Let There Be Pickups](https://aibodh.com/posts/bevy-rust-game-development-chapter-5/)。两种模式都可自动拾取草药、花朵、蘑菇和蕨类：走进物品周围 40 像素（3D 中 1.25 格）的范围即可。拾取后物品从地图消失，左上角背包计数立即更新；换角不清空背包，退出游戏后不保留。

2D 镜头放大到约 10 格高并平滑跟随。这里调整正交投影而非把瓦片改成 64 像素，以保持原有速度、碰撞和地图单位。3D 保留鼠标环绕和障碍避让，跟随位置与镜头拉回后的复位改为平滑过渡。两个模式都可加 `--fullscreen` 使用无边框全屏。

## 第四章：状态、碰撞和遮挡

- `Esc` 暂停/继续，暂停时停止移动、动画、换角和相机控制。加载时先读取角色配置与贴图（2D 还会等待地形图集），离开加载状态时只创建一次玩家。
- 角色使用 Idle、Walking、Running、Jumping 四种互斥状态；播放跳跃动画时不做水平移动。输入计算速度，移动系统再按该速度处理碰撞。
- 两种视图从 WFC 模型生成同一个碰撞网格。临接陆地的水格为浅岸；树冠不参与地面碰撞。角色用脚下的圆形碰撞体，分小步扫掠以防穿透，斜撞时可沿边滑动。
- `F3` 切换碰撞调试：青色圆是玩家脚下碰撞体，红框是树干/岩石/树桩，蓝框是深水，绿框是可走浅岸。2D 按脚底与物件基座的 Y 排序，树冠沿用树干基座的深度；3D 使用深度缓冲遮挡，并保留第三人称镜头的避障。

## 第三章：数据驱动角色

1. `src/assets/characters/characters.ron` 定义六个角色的速度、奔跑倍率、最大生命值和图集动画。生命值目前只记录在配置和切换日志里。修改配置后重启即可看到效果。
2. `characters/config.rs` 使用 Bevy `AssetLoader` 异步读取 RON，并校验动画行、帧数和时长；不需要额外的第三方 loader。当前角色图集为 9 列 × 12 行，帧大小为 64×64 像素。Walk/Run/Jump 每组四行，顺序为上、左、下、右；配置也支持单行的非方向动画。
3. `characters/motion.rs` 负责输入、移动和动画计时；切换 Walk/Run 或转向时重开循环。Jump 只播放一遍，转向不重置进度；结束时回到当前输入决定的 Walk/Run。切换角色保留平面位置与朝向，清除旧动画和跳跃状态。
4. `characters/view.rs` 在移动更新后读取同一帧：2D 通过 `Sprite` 图集索引显示；3D 使用始终面向相机的透明像素面片，通过材质 UV 选帧，仍受地形深度遮挡。3D 把 2D 的向上移动映射为 `-Z`，并将像素距离除以 32；跳跃抬升 2D 的 `Y` 或 3D 的高度 `Y`。

## 3D 版如何复用 2D 的地图

1. `main.rs` 选择 `MapPlugin::Blocks3d` 或 `MapPlugin::Sprites2d`。
2. 两个模式都调用 `map/generate.rs` 的 `create_generator`，共用同一套模型、连接规则和种子。生成的是地图数据，不是图像。
3. 2D 版按图集生成精灵。3D 版按每个模型的首个素材名选择外观，并把网格 `(x, y)` 映射到世界 `(X, Z)`。陆地取最上层的可见地表，水格用低于岸边的蓝色水面覆盖泥床；边角不复制 2D 图集的轮廓。
4. `map/block_art.rs` 在启动时生成原创 16×16 像素纹理，用最近邻采样保持像素边缘。所有方块共享一个立方体网格，UV 分开采样顶面、侧面和底面，因此草方块的侧面能显示草沿与泥土。树、岩石和花由多个小方块组合。

这是同一张分层瓦片地图的 3D 表现，不是可挖掘的体素地形。水面不透明，不含水流模拟；浅岸水面比深水更接近陆面。

## 文件

```text
src/main.rs                       App / 插件 / 资源路径 / 相机
src/characters/                   角色配置加载 / 共用控制器 / 2D、3D 渲染
src/assets/characters/            characters.ron 与 6 张原创角色图集
src/map/                          资源 / 模型 / sockets / 规则 / 生成与测试
src/map/view_3d.rs                方块组合 / 世界坐标 / 3D 相机与灯光
src/map/block_art.rs              原创像素纹理 / 顶侧底 UV / 材质
src/assets/tile_layers/tilemap.png George Bailey 的地形图集（CC-BY 4.0）
src/assets/tile_layers/CREDITS.md  地形图集作者和许可
tools/generate_sprite.py           仅依赖 Python 标准库的人物资源生成脚本
LICENSE-MIT                       教程代码原始 MIT 声明
```

重新生成图片：

```sh
python3 practice/p03_aibodh_chapter1/tools/generate_sprite.py
```

## 资源和代码来源

代码参考 [ImpatientProgrammerBevyRust / chapter1](https://github.com/jamesfebin/ImpatientProgrammerBevyRust/tree/37b5849327984c4378498d875e3c428bacb25c0c/chapter1)，作者 Febin John James，保留其 MIT 声明于 `LICENSE-MIT`。

**本工程没有复制教程的 LPC 人物图片。** 上游仓库明确授权教程代码为 MIT，但没有提供原图的完整 LPC 图层作者、credits 或生成配置。为避免将代码许可误当作美术许可，这里用生成脚本绘制原创像素人物，保留相同的教学布局。角色外观因此与文章不同。

`tools/generate_sprite.py` 及生成的六张角色图集按 MIT 许可提供，使用本目录 `LICENSE-MIT` 的条款；不需要外部美术资源、Python 图片库或网络下载。

## 第二章：地图生成的阅读顺序

1. `map/tilemap.rs`：将 sprite 名字对应到图集中 32×32 的矩形。
2. `map/assets.rs`：`SpawnableAsset` 用 sprite 名、网格偏移、世界偏移和可选的拾取类型描述生成结果；多个 sprite 可组成一棵树。
3. `map/models.rs`：`TerrainModelBuilder` 将模型和 sprite 绑定放在同一索引，避免错配。
4. `map/sockets.rs`：创建水平材料、边缘和上下层 socket。
5. `map/rules.rs`：泥土 → 绿草 → 黄草 → 水 → 装饰五层；透明模型表示该层空白。黄草只能在完整绿草上，装饰物底部只能连接水层的透明陆地；大树左右基座必须配对。
6. `map/generate.rs`：用库执行 WFC，再通过库的 `spawn_node` 将结果生成到地图父实体下。

### 配置与实现约定

- `generate.rs` 的 `MAP_SEED = 42`：修改数字并重启可得到另一张可复现的地图。固定种子不保证所有种子都能在重试预算内成功。
- `GRID_X/GRID_Y` 控制地图宽高，`GRID_Z = 5` 对应规则中的五层。
- `rules.rs` 的 `WATER_WEIGHT = 0.07` 保留现有小地图的水密度，让默认种子同时展示浅岸与深水；它不是精确百分比。
- 小地图在 `Startup` 同步生成，最多重试 50 次，失败明确报错。没有使用库中失败后逐帧无限重试的 simple plugin；生成核心仍是教程指定的 WFC 库。
- 装饰层外围固定为空白，防止边界生成半棵大树；树干的上方通过 `canopy_space` socket 要求空白装饰格，避免树冠与其他装饰重叠。树冠可延伸到水面上，但树干基座仍只在陆地。
- 地表固定按层叠放；只有装饰和角色进入以基座 Y 排序的绘制层。
- WFC 保证局部连接规则，不保证一定出现湖泊、所有装饰种类或可通行路线。

### 地形图集许可

第二章地图代码参考同一固定版本的上游 `chapter2/src/map`，保留 MIT 声明。

`tilemap.png` 来自教程第二章图集，基于 [George Bailey 的 16x16 Game Assets](https://opengameart.org/content/16x16-game-assets)，许可为 **CC-BY 4.0，不是 MIT**。原作者、来源、上游重新排版信息及许可链接见 `src/assets/tile_layers/CREDITS.md`；分发游戏时也应提供可访问的署名。本工程未修改该图集。

## 测试范围

角色测试在无窗口 Bevy App 中加载真实 RON，验证两种模式的移动、换角、选帧及一次性跳跃，并核对角色 PNG 的图集尺寸。地图测试验证可复现性、地形与贴图，以及浅岸/深水、穿透防护、滑动和安全出生点；角色测试也覆盖暂停和恢复。实际 GPU 渲染和键盘手感仍需启动验证。
