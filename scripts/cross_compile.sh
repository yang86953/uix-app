#!/usr/bin/env bash

# 任一目标、依赖或 Cargo 子命令失败时立即结束本次构建。
set -Eeuo pipefail

# 从脚本位置解析唯一仓库根，不依赖调用方当前目录。
readonly cross_repo_root="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd -P)"
# 冻结仓库交叉链接 Windows 所验证的 cargo-xwin 版本。
readonly cross_cargo_xwin_version="0.23.1"
# 冻结当前产品与预备桌面平台的 Rust 目标三元组。
readonly cross_linux_x64_target="x86_64-unknown-linux-gnu"
readonly cross_windows_x64_target="x86_64-pc-windows-msvc"
readonly cross_macos_arm64_target="aarch64-apple-darwin"
readonly cross_macos_x64_target="x86_64-apple-darwin"
# macOS 构建只选择原生 Metal 与当前公开组件能力，不借用 Windows/Linux 后端。
readonly cross_macos_features="application,metal,agent-control,image-codecs"

# 输出稳定错误并终止，避免部分目标通过时被误认为完整矩阵成功。
cross_fail() {
    printf 'UIX cross compilation failed: %s\n' "$*" >&2
    exit 1
}

# 输出唯一命令行契约。
cross_usage() {
    cat <<'EOF'
Usage:
  bash scripts/cross_compile.sh check [all|linux-x64|windows-x64|macos-arm64|macos-x64] [dev|release]
  bash scripts/cross_compile.sh build [linux-x64|windows-x64|macos-arm64|macos-x64] [dev|release]

check 可在任意宿主执行 Rust 目标编译检查；build 必须具备目标链接环境：
  linux-x64   Linux x64 宿主
  windows-x64 Windows MSVC 宿主，或安装 cargo-xwin 0.23.1 与 LLVM 的非 Windows 宿主
  macos-*     安装 Xcode Command Line Tools / Apple SDK 的 macOS 宿主
EOF
}

# 要求构建入口依赖的命令真实存在。
cross_require_command() {
    command -v "$1" >/dev/null 2>&1 || cross_fail "missing command: $1"
}

# 要求 rustup 已为当前工具链安装目标标准库。
cross_require_target() {
    rustup target list --installed | grep -Fx "$1" >/dev/null \
        || cross_fail "Rust target is not installed: $1 (run: rustup target add $1)"
}

# 返回当前 rustc 宿主三元组。
cross_host_target() {
    # 完整消费 rustc 输出，避免 pipefail 下读取端提前退出触发 SIGPIPE。
    rustc -vV | sed -n 's/^host:[[:space:]]*//p'
}

# 对根库和主演示执行 Windows/Linux 当前交付配置的编译检查。
cross_check_release_platform() {
    local cross_target="$1"
    (
        cd "$cross_repo_root"
        cargo check --locked --package uix-app --target "$cross_target" \
            --features agent-control "${cross_profile_args[@]}"
        cargo check --locked --manifest-path "$cross_repo_root/../uix-widgets/Cargo.toml" --example showcase \
            --target "$cross_target" --features examples,agent-control,vulkan,opengles,d3d11 "${cross_profile_args[@]}"
    )
}

# 对根库默认图与原生 Metal 图、主演示原生 Metal 图执行 macOS 编译检查。
cross_check_macos() {
    local cross_target="$1"
    (
        cd "$cross_repo_root"
        cargo check --locked --package uix-app --target "$cross_target" \
            --features agent-control "${cross_profile_args[@]}"
        cargo check --locked --package uix-app --target "$cross_target" \
            --no-default-features --features "$cross_macos_features" \
            "${cross_profile_args[@]}"
        cargo check --locked --manifest-path "$cross_repo_root/../uix-widgets/Cargo.toml" --example showcase \
            --target "$cross_target" --no-default-features \
            --features examples,agent-control,metal \
            "${cross_profile_args[@]}"
    )
}

# 在 Linux x64 宿主链接当前 Linux 交付配置。
cross_build_linux_x64() {
    local cross_host
    cross_host="$(cross_host_target)"
    [[ "$cross_host" == "$cross_linux_x64_target" ]] \
        || cross_fail "linux-x64 linking requires a $cross_linux_x64_target host; current host is $cross_host"
    (
        cd "$cross_repo_root"
        cargo build --locked --package uix-app --target "$cross_linux_x64_target" \
            --features agent-control "${cross_profile_args[@]}"
        cargo build --locked --manifest-path "$cross_repo_root/../uix-widgets/Cargo.toml" --example showcase \
            --target "$cross_linux_x64_target" --features examples,agent-control,vulkan,opengles,d3d11 \
            "${cross_profile_args[@]}"
    )
}

# 在 Windows 原生宿主或 cargo-xwin 提供的 MSVC sysroot 上链接 PE/COFF 产物。
cross_build_windows_x64() {
    local cross_host
    local cross_xwin_version
    local -a cross_windows_driver
    cross_host="$(cross_host_target)"
    if [[ "$cross_host" == "$cross_windows_x64_target" ]]; then
        cross_windows_driver=(cargo)
    else
        cross_require_command clang-cl
        cross_require_command lld-link
        cross_require_command llvm-lib
        cross_xwin_version="$(cargo xwin --version 2>/dev/null || true)"
        [[ "$cross_xwin_version" == "cargo-xwin-xwin $cross_cargo_xwin_version" \
            || "$cross_xwin_version" == "cargo-xwin $cross_cargo_xwin_version" ]] \
            || cross_fail "cargo-xwin $cross_cargo_xwin_version is required; observed: ${cross_xwin_version:-not installed}"
        # 只取得桌面 x64 CRT/SDK，避免缓存未承诺的 Windows 架构。
        export XWIN_ARCH=x86_64
        export XWIN_VARIANT=desktop
        export XWIN_VERSION=17
        cross_windows_driver=(cargo xwin)
    fi
    (
        cd "$cross_repo_root"
        "${cross_windows_driver[@]}" build --locked --package uix-app \
            --target "$cross_windows_x64_target" --features agent-control \
            "${cross_profile_args[@]}"
        "${cross_windows_driver[@]}" build --locked --manifest-path "$cross_repo_root/../uix-widgets/Cargo.toml" \
            --example showcase --target "$cross_windows_x64_target" \
            --features examples,agent-control,vulkan,opengles,d3d11 "${cross_profile_args[@]}"
    )
}

# 只在持有合法 Apple SDK 的 macOS 宿主链接对应架构产物。
cross_build_macos() {
    local cross_target="$1"
    local cross_host
    cross_host="$(cross_host_target)"
    [[ "$cross_host" == *-apple-darwin ]] \
        || cross_fail "macOS linking requires a macOS host with Apple SDK; current host is $cross_host"
    cross_require_command xcrun
    xcrun --show-sdk-path >/dev/null 2>&1 \
        || cross_fail "xcrun cannot resolve the macOS SDK"
    (
        cd "$cross_repo_root"
        cargo build --locked --package uix-app --target "$cross_target" \
            --no-default-features --features "$cross_macos_features" \
            "${cross_profile_args[@]}"
        cargo build --locked --manifest-path "$cross_repo_root/../uix-widgets/Cargo.toml" --example showcase \
            --target "$cross_target" --no-default-features \
            --features examples,agent-control,metal \
            "${cross_profile_args[@]}"
    )
}

[[ $# -le 3 ]] || {
    cross_usage >&2
    exit 2
}
readonly cross_mode="${1:-check}"
readonly cross_platform="${2:-all}"
readonly cross_profile="${3:-dev}"

case "$cross_mode" in
    check|build) ;;
    -h|--help|help)
        cross_usage
        exit 0
        ;;
    *)
        cross_usage >&2
        cross_fail "unknown mode: $cross_mode"
        ;;
esac

case "$cross_platform" in
    all|linux-x64|windows-x64|macos-arm64|macos-x64) ;;
    *)
        cross_usage >&2
        cross_fail "unknown platform: $cross_platform"
        ;;
esac

case "$cross_profile" in
    dev) cross_profile_args=() ;;
    release) cross_profile_args=(--release) ;;
    *)
        cross_usage >&2
        cross_fail "unknown profile: $cross_profile"
        ;;
esac

if [[ "$cross_mode" == "build" && "$cross_platform" == "all" ]]; then
    cross_fail "build mode requires an explicit platform because Linux, Windows, and macOS have different linker/SDK owners"
fi

cross_require_command cargo
cross_require_command rustc
cross_require_command rustup

if [[ "$cross_mode" == "check" ]]; then
    case "$cross_platform" in
        all)
            cross_require_target "$cross_linux_x64_target"
            cross_require_target "$cross_windows_x64_target"
            cross_require_target "$cross_macos_arm64_target"
            cross_require_target "$cross_macos_x64_target"
            cross_check_release_platform "$cross_linux_x64_target"
            cross_check_release_platform "$cross_windows_x64_target"
            cross_check_macos "$cross_macos_arm64_target"
            cross_check_macos "$cross_macos_x64_target"
            ;;
        linux-x64)
            cross_require_target "$cross_linux_x64_target"
            cross_check_release_platform "$cross_linux_x64_target"
            ;;
        windows-x64)
            cross_require_target "$cross_windows_x64_target"
            cross_check_release_platform "$cross_windows_x64_target"
            ;;
        macos-arm64)
            cross_require_target "$cross_macos_arm64_target"
            cross_check_macos "$cross_macos_arm64_target"
            ;;
        macos-x64)
            cross_require_target "$cross_macos_x64_target"
            cross_check_macos "$cross_macos_x64_target"
            ;;
    esac
else
    case "$cross_platform" in
        linux-x64)
            cross_require_target "$cross_linux_x64_target"
            cross_build_linux_x64
            ;;
        windows-x64)
            cross_require_target "$cross_windows_x64_target"
            cross_build_windows_x64
            ;;
        macos-arm64)
            cross_require_target "$cross_macos_arm64_target"
            cross_build_macos "$cross_macos_arm64_target"
            ;;
        macos-x64)
            cross_require_target "$cross_macos_x64_target"
            cross_build_macos "$cross_macos_x64_target"
            ;;
    esac
fi

printf 'UIX cross compilation passed: mode=%s platform=%s profile=%s\n' \
    "$cross_mode" "$cross_platform" "$cross_profile"
