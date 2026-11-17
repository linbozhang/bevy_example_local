#!/usr/bin/env bash
# 从本地 bevy-fork-pinned (0.20) 运行官方 example，保证 example 与源码版本一致
#
# 用法:
#   ./run_example.sh 3d/3d_scene
#   ./run_example.sh ecs/ecs_guide
#   ./run_example.sh hello_world
#   ./run_example.sh 3d/3d_scene --release        # 额外参数原样透传给 cargo
#
# 说明:
#   - fork 路径默认取 /Users/linbo.zhang/workspace/github/bevy-fork-pinned，
#     可用环境变量 BEVY_FORK 覆盖
#   - example 名不带 .rs 后缀；子目录用 / 分隔（对应 fork 仓库 examples/ 下的相对路径）
#   - 首次运行要在 fork 里编译整个 bevy，会比较久；之后增量编译很快

set -euo pipefail

BEVY_FORK="${BEVY_FORK:-/Users/linbo.zhang/workspace/github/bevy-fork-pinned}"

if [ $# -lt 1 ]; then
    echo "用法: $0 <example路径> [额外cargo参数...]" >&2
    echo "例如: $0 3d/3d_scene" >&2
    exit 1
fi

EXAMPLE="$1"
shift

cd "$BEVY_FORK"
exec cargo run --example "$EXAMPLE" "$@"
