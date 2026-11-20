# 创建新的 practice

本项目使用 Cargo workspace（工作区）管理练习：每个练习是一个独立的可执行包，根目录的 `Cargo.toml` 统一管理 Bevy 版本和编译优化配置。

> `practice/p01_first_scene` 已经创建好，直接编辑它的 `src/main.rs` 即可，不要重复初始化。

## 1. 用 Cargo 创建练习

在项目根目录 `bevy_example_local` 的终端中执行下面的命令。这里以 `p02_demo` 为例，创建自己的练习时替换这个名字：

```sh
cargo new practice/p02_demo --bin --vcs none
```

- `practice/p02_demo`：创建新包的目录，默认包名为 `p02_demo`。
- `--bin`：创建可执行程序，入口是 `src/main.rs`。
- `--vcs none`：不在练习目录内创建独立 Git 仓库，使用项目外层已有的仓库。

Cargo 会自动生成目录和文件，不需要手动创建：

```text
practice/p02_demo/
├── Cargo.toml
└── src/
    └── main.rs
```

如果目录已经存在，但还没有初始化成 Cargo 包，改用：

```sh
cargo init practice/p02_demo --bin --vcs none
```

已有 `Cargo.toml` 的练习不需要再次执行 `cargo new` 或 `cargo init`。

## 2. 检查工作区成员

在当前工作区内创建新包时，Cargo 通常会自动将它加入根目录 `Cargo.toml` 的 `members`。检查新练习是否已经列出；如果没有，手动添加，保留其他已有成员：

```toml
[workspace]
resolver = "2"
members = [
    "practice/p01_first_scene",
    "practice/p02_demo",
]
```

根目录已有的 `[workspace.dependencies]` 和 `[profile.dev]` 等配置保持不变。

## 3. 配置 Bevy 依赖

打开新练习的 `practice/p02_demo/Cargo.toml`，在已有的 `[dependencies]` 下添加：

```toml
[dependencies]
bevy.workspace = true
```

`bevy.workspace = true` 表示继承根目录 `[workspace.dependencies]` 中的 Bevy 配置，不需要在每个练习里重复填写版本号。

保留 Cargo 生成的 `[package]` 配置，不要重复添加 `[dependencies]` 表头。

如果需要使用与 p01 相同的动态链接选项，再添加：

```toml
[features]
dynamic_linking = ["bevy/dynamic_linking"]
```

## 4. 编辑代码

Cargo 默认生成的是 Hello World，而不是 Bevy 场景。将新练习的 `src/main.rs` 替换为以下最小启动代码，然后继续编写自己的系统和场景：

```rust
use bevy::prelude::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .run();
}
```

这段代码会启动 Bevy 应用，但尚未添加相机或场景实体；要显示场景，需要继续添加对应的初始化系统。

## 5. 检查和运行

以下命令都在项目根目录执行。

先检查代码能否通过编译：

```sh
cargo check -p p02_demo
```

运行新练习：

```sh
cargo run -p p02_demo
```

`-p` 后面填写的是新练习 `Cargo.toml` 中的 `[package].name`，不是目录路径。

如果在第 3 步添加了 `dynamic_linking` feature，也可以这样运行：

```sh
cargo run -p p02_demo --features dynamic_linking
```

首次构建需要下载和编译依赖，可能耗时较长。工作区内的练习默认共享根目录的 `target/` 构建目录；配置兼容时可以复用依赖编译产物。动态链接可以减少后续链接开销，但不保证首次编译更快。

## 快速回顾

1. `cargo new practice/练习名 --bin --vcs none` 创建包。
2. 检查根 `Cargo.toml` 的 `members` 是否包含新练习。
3. 在新包中添加 `bevy.workspace = true`。
4. 编辑新包的 `src/main.rs`。
5. 用 `cargo check -p 练习名` 检查，用 `cargo run -p 练习名` 运行。
