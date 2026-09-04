#!/usr/bin/env bash

# 任何失败、未定义变量或管道中间失败都立即终止构建。
set -Eeuo pipefail

# 固定文本排序、工具输出与归档元数据使用的区域设置。
export LC_ALL=C
# 固定 ZIP 条目时间解释使用 UTC。
export TZ=UTC

# 解析脚本所在仓库的规范绝对路径。
readonly release_repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
# 固定当前内部候选版本。
readonly release_version="0.0.7"
# 固定 Windows x64 候选包名称。
readonly release_artifact_name="uix-${release_version}-internal-win-x64"
# 固定 Windows MSVC 目标。
readonly release_windows_target="x86_64-pc-windows-msvc"

# 输出稳定错误并终止构建。
release_fail() {
    # 将发布失败原因写入标准错误。
    printf 'Windows cross internal release failed: %s\n' "$*" >&2
    # 使用非零状态结束当前进程。
    exit 1
}

# 验证确定性构建依赖全部存在。
for release_command in bash cargo git install jq mktemp python3 realpath sha256sum touch uname zip; do
    # 缺少任一工具都不能继续生成候选包。
    command -v "$release_command" >/dev/null 2>&1 || release_fail "missing command: $release_command"
done

# 本入口只负责从 Linux x86_64 宿主交叉链接 Windows x64 候选包。
[[ "$(uname -s)" == "Linux" ]] || release_fail "host operating system must be Linux"
# 当前交叉交付契约只覆盖 x86_64 宿主。
[[ "$(uname -m)" == "x86_64" ]] || release_fail "host architecture must be x86_64"

# 读取包含全部未跟踪文件的工作树状态。
release_dirty_entries="$(git -C "$release_repo_root" status --porcelain=v1 --untracked-files=all)"
# 内部候选包只能从干净工作树生成。
[[ -z "$release_dirty_entries" ]] || release_fail "the internal release must be built from a clean worktree"

# 固定两个独立 Cargo workspace 的锁文件路径。
readonly release_root_lock="$release_repo_root/Cargo.lock"
readonly release_demo_lock="$release_repo_root/demo/Cargo.lock"
# 只清理本次为干净源码生成的锁文件，不触碰使用方既有忽略文件。
release_root_lock_generated=false
release_demo_lock_generated=false
release_cleanup_generated_locks() {
    # Demo 锁文件由本次生成时才允许删除。
    if [[ "$release_demo_lock_generated" == true ]]; then
        # 精确回收 Demo 锁文件。
        rm -f -- "$release_demo_lock"
    fi
    # 根锁文件由本次生成时才允许删除。
    if [[ "$release_root_lock_generated" == true ]]; then
        # 精确回收根锁文件。
        rm -f -- "$release_root_lock"
    fi
}
# 在发布临时根建立前发生任何错误，也必须回收本次生成的锁文件。
trap release_cleanup_generated_locks EXIT

# 干净发布源先解析锁定图，随后构建全程使用 --locked。
if [[ ! -f "$release_root_lock" ]]; then
    # 标记根锁文件由本次构建拥有。
    release_root_lock_generated=true
    # 生成根工作区锁定图。
    cargo generate-lockfile --manifest-path "$release_repo_root/Cargo.toml"
fi
if [[ ! -f "$release_demo_lock" ]]; then
    # 标记 Demo 锁文件由本次构建拥有。
    release_demo_lock_generated=true
    # 生成 Demo 工作区锁定图。
    cargo generate-lockfile --manifest-path "$release_repo_root/demo/Cargo.toml"
fi

# 从锁定依赖图读取根工作区元数据。
release_metadata="$(cd "$release_repo_root" && cargo metadata --format-version 1 --no-deps --locked)"
# 验证恰好存在一个匹配版本的 uix package。
release_package_count="$(jq --arg version "$release_version" '[.packages[] | select(.name == "uix" and .version == $version)] | length' <<<"$release_metadata")"
# 拒绝缺失或重复的根 package 身份。
[[ "$release_package_count" == "1" ]] || release_fail "expected exactly one uix package at version $release_version"
# 读取内部 package 描述。
release_package_description="$(jq -r --arg version "$release_version" '.packages[] | select(.name == "uix" and .version == $version) | .description // ""' <<<"$release_metadata")"
# 描述必须包含至少一个非空白字符。
[[ "$release_package_description" =~ [^[:space:]] ]] || release_fail "expected the uix package to declare a non-empty description"
# 读取并规范化 Cargo 根工作区产物目录。
release_target_root="$(realpath -m "$(jq -r '.target_directory' <<<"$release_metadata")")"
# 只允许把候选包写入当前仓库内部。
case "$release_target_root" in
    # 仓库内部目录可以继续使用。
    "$release_repo_root"/*) ;;
    # 其他目录一律拒绝。
    *) release_fail "refusing an internal release target outside the repository: $release_target_root" ;;
esac

# 复用统一交叉编译入口链接根库与带 Agent 能力的 Windows 主演示。
bash "$release_repo_root/scripts/cross_compile.sh" build windows-x64 release
# 解析内部过程宏 crate 的规范路径。
release_derive_path="$(realpath "$release_repo_root/uix-derive")"
# 生成不从外部 registry 解析私有 uix-derive 的锁定内部 cargo package。
(cd "$release_repo_root" && cargo package --locked --config "patch.crates-io.uix-derive.path='${release_derive_path}'")

# 固定 Windows 主演示产物路径。
readonly release_demo_path="$release_repo_root/demo/target/$release_windows_target/release/uix-lang-demo.exe"
# 固定内部 crate 产物路径。
readonly release_crate_path="$release_target_root/package/uix-${release_version}.crate"
# 固定内部候选输出目录。
readonly release_output_root="$release_target_root/internal-release"
# 创建受仓库 target 边界保护的输出目录。
mkdir -p -- "$release_output_root"
# 在输出目录内创建本次构建唯一临时根。
release_temp_root="$(mktemp -d "$release_output_root/.${release_artifact_name}.XXXXXX")"
# 固定临时 staging 目录。
readonly release_stage_root="$release_temp_root/stage"
# 固定临时候选包路径，并保留最终规范文件名。
readonly release_temp_archive="$release_temp_root/${release_artifact_name}.zip"
# 固定最终候选包路径。
readonly release_archive_path="$release_output_root/${release_artifact_name}.zip"

# 清理仅由本次构建创建的临时目录。
release_cleanup() {
    # 临时根必须仍位于预期输出目录内。
    case "$release_temp_root" in
        # 只删除带固定前缀的本次临时根。
        "$release_output_root"/."$release_artifact_name".*) rm -rf -- "$release_temp_root" ;;
        # 边界异常时保留现场并报告。
        *) printf 'Refusing to clean unexpected temporary path: %s\n' "$release_temp_root" >&2 ;;
    esac
    # 临时归档目录回收后，再精确移除本次生成的锁文件。
    release_cleanup_generated_locks
}
# 无论成功或失败都回收本次临时根。
trap release_cleanup EXIT
# 创建 staging 根目录。
mkdir -p -- "$release_stage_root"

# 固化归档内的七个 payload 名称及顺序。
release_payload_names=(
    # Windows 主演示使用规范 exe 名称。
    'uix-lang-demo.exe'
    # 内部 crate 使用精确版本命名。
    "uix-${release_version}.crate"
    # 专有许可随包交付。
    'LICENSE'
    # 第三方声明随包交付。
    'THIRD_PARTY_NOTICES.md'
    # 当前版本变更记录随包交付。
    'CHANGELOG.md'
    # 使用入口随包交付。
    'README.md'
    # Demo 运行时图片使用规范相对路径。
    'assets/images/demo.png'
)
# 按相同下标固定每个 payload 的源文件。
release_payload_sources=(
    # Windows 交叉链接的主演示二进制。
    "$release_demo_path"
    # 根 crate 打包产物。
    "$release_crate_path"
    # 仓库专有许可。
    "$release_repo_root/LICENSE"
    # 仓库第三方声明。
    "$release_repo_root/THIRD_PARTY_NOTICES.md"
    # 仓库变更记录。
    "$release_repo_root/CHANGELOG.md"
    # 仓库使用入口。
    "$release_repo_root/README.md"
    # 主演示运行时图片。
    "$release_repo_root/assets/images/demo.png"
)

# 逐项复制冻结载荷，并保持稳定的归档顺序。
for release_index in "${!release_payload_names[@]}"; do
    # 读取当前规范名称。
    release_name="${release_payload_names[$release_index]}"
    # 读取当前源文件。
    release_source="${release_payload_sources[$release_index]}"
    # 所有载荷必须是现有普通文件。
    [[ -f "$release_source" ]] || release_fail "missing release payload: $release_source"
    # ZIP 不承诺 Unix 执行位，staging 统一使用只读共享模式。
    install -D -m 0644 -- "$release_source" "$release_stage_root/$release_name"
done

# 固定摘要清单路径。
readonly release_manifest_path="$release_stage_root/SHA256SUMS.txt"
# 创建空的 ASCII 摘要清单。
: >"$release_manifest_path"
# 按 payload 冻结顺序生成摘要。
for release_name in "${release_payload_names[@]}"; do
    # 计算当前解包载荷的小写 SHA-256。
    release_hash="$(sha256sum -- "$release_stage_root/$release_name" | awk '{print $1}')"
    # 使用两个空格和 LF 写入规范摘要行。
    printf '%s  %s\n' "$release_hash" "$release_name" >>"$release_manifest_path"
done

# 固化完整八项归档顺序。
release_archive_entries=(
    # 先写入七个 payload。
    "${release_payload_names[@]}"
    # 最后写入不自哈希的清单。
    'SHA256SUMS.txt'
)
# 把所有条目时间冻结为 ZIP 规范允许的最早时间。
for release_name in "${release_archive_entries[@]}"; do
    # 只修改 staging 副本，不触碰仓库源文件。
    touch -t 198001010000.00 -- "$release_stage_root/$release_name"
done
# 排除额外字段并按显式顺序生成确定性 ZIP。
(cd "$release_stage_root" && zip -X -q -9 "$release_temp_archive" "${release_archive_entries[@]}")
# 用独立校验器验证精确结构、元数据和逐项摘要。
python3 "$release_repo_root/scripts/verify_internal_release.py" --archive "$release_temp_archive" --version "$release_version"
# 校验成功后原子替换可重建的最终候选包。
mv -f -- "$release_temp_archive" "$release_archive_path"
# 计算最终容器摘要。
release_archive_hash="$(sha256sum -- "$release_archive_path" | awk '{print $1}')"
# 输出候选包路径。
printf 'Artifact: %s\n' "$release_archive_path"
# 输出最终 SHA-256。
printf 'SHA256:   %s\n' "$release_archive_hash"
