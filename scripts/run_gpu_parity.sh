#!/usr/bin/env bash
# GPU 离屏像素一致性验证的仓库专用入口（不占用 crates.io 发布包的公开 feature）。
# 用法：scripts/run_gpu_parity.sh {vulkan|opengl|d3d11} [额外 cargo test 参数...]
# 三个专用 cfg 键只在本脚本显式设置，普通构建与发布包内恒为关闭，无悬空模块。
set -euo pipefail

scene="${1:?用法: run_gpu_parity.sh 场景[vulkan|opengl|d3d11] [额外 cargo test 参数...]}"
shift

case "$scene" in
    vulkan)
        # 原 vulkan-parity-test 组合：Vulkan 后端在默认特性中，不依赖 test-harness。
        cfg_key=uix_gpu_parity_vulkan
        features=""
        ;;
    opengl)
        # 原 opengl-parity-test 组合：EGL/GLES harness 观察计数在 test-harness 门控下。
        cfg_key=uix_gpu_parity_opengl
        features=test-harness
        ;;
    d3d11)
        # 原 d3d11-parity-test 组合；生产 Adapter 仅在 Windows 目标编译。
        cfg_key=uix_gpu_parity_d3d11
        features=test-harness
        ;;
    *)
        echo "未知场景: $scene（可选 vulkan|opengl|d3d11）" >&2
        exit 2
        ;;
esac

if [[ "$(uname -s)" == Linux && "$scene" == d3d11 ]]; then
    echo "d3d11 parity 需在 Windows 目标运行（当前为 Linux，仅可做交叉检查编译）。" >&2
fi

# 与仓库其余验证一致：显式本仓 target、双任务并行、稳定工具链。
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$(cd "$(dirname "$0")/.." && pwd)/target}"
if [[ -n "$features" ]]; then
    exec env RUSTFLAGS="--cfg ${cfg_key}" cargo test -p uix-app --lib --features "$features" -j2 "$@"
else
    exec env RUSTFLAGS="--cfg ${cfg_key}" cargo test -p uix-app --lib -j2 "$@"
fi
