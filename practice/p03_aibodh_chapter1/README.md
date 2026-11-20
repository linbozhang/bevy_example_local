# p03_aibodh_chapter1 — Let There Be a Player

按 [AIBodh 第一章](https://aibodh.com/posts/bevy-rust-game-development-chapter-1/) 的最终版本实现，使用工作区 Bevy 0.20.1：白色背景、2D 相机、像素人物、方向键移动、四方向行走动画、独立 `PlayerPlugin`。不是前一个 RPG 工程的续作，不包含战斗。

## 运行

在 workspace 根目录：

```sh
cargo run -p p03_aibodh_chapter1
cargo test -p p03_aibodh_chapter1
```

也可以进入本目录后运行 `cargo run`。资源目录由 `CARGO_MANIFEST_DIR` 定位，启动时不必复制资源。该路径适合源码练习；若单独分发可执行文件，需要改为可部署的资源路径。

- 方向键：移动；松键停止并保留当前动画姿势。
- 速度：140 像素/秒；斜向移动归一化，不会更快。
- 动画：每方向 9 帧，0.1 秒/帧。
- 关闭窗口：退出。
- 相机不跟随、没有边界和碰撞；人物可以走出画面，这是第一章的范围。

## 文件

```text
src/main.rs                       App / 插件 / 资源路径 / 相机
src/player.rs                     PlayerPlugin / 组件 / 移动 / 动画 / 测试
src/assets/player_spritesheet.png 原创像素人物图集
 tools/generate_sprite.py          仅依赖 Python 标准库的资源生成脚本
LICENSE-MIT                       教程代码原始 MIT 声明
```

重新生成图片：

```sh
python3 practice/p03_aibodh_chapter1/tools/generate_sprite.py
```

## 与文章相比的必要修正

1. **图集步长与动画帧数分开。** 教程仓库原图是 832×3456，即 13 列、54 行的 64×64 图块；但文章按 9 列定义图集，会截错帧。本工程生成同样尺寸的图，使用 13 列图集，行走仅循环每行前 9 帧。Up/Left/Down/Right 使用行 8/9/10/11。
2. **移动先于动画。** `(move_player, animate_player).chain()` 确定执行顺序，不能靠共享可写组件自动推断先后。
3. **像素图使用 nearest 采样。** 避免线性采样把人物边缘变模糊。
4. **低帧率动画补帧。** 使用 `times_finished_this_tick()`，一个更新跨过多个间隔时不会只推进一帧。
5. `Camera2d` 是组件，不是旧式 camera bundle；`texture_atlas == None` 表示未配置图集，不代表图片仍在后台加载。

## 资源和代码来源

代码参考 [ImpatientProgrammerBevyRust / chapter1](https://github.com/jamesfebin/ImpatientProgrammerBevyRust/tree/37b5849327984c4378498d875e3c428bacb25c0c/chapter1)，作者 Febin John James，保留其 MIT 声明于 `LICENSE-MIT`。

**本工程没有复制教程的 LPC 人物图片。** 上游仓库明确授权教程代码为 MIT，但没有提供原图的完整 LPC 图层作者、credits 或生成配置。为避免将代码许可误当作美术许可，这里用生成脚本绘制原创像素人物，保留相同的教学布局。角色外观因此与文章不同。

`tools/generate_sprite.py` 及生成的 `player_spritesheet.png` 按 MIT 许可提供，使用本目录 `LICENSE-MIT` 的条款；不需要外部美术资源、Python 图片库或网络下载。

## 测试范围

测试在无窗口的 Bevy App 中运行真实移动/动画系统：图集与 PNG 尺寸一致、四方向、同帧动画更新、斜向速度、相反按键抵消、动画循环/补帧、停止姿势、连续转向。窗口显示、GPU 渲染和真实键盘输入还需实际启动验证。
