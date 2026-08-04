#!/usr/bin/env python3
"""采集使用方 Rust fixture 的 clean-build 与 compile-fail 证据。"""

# 启用未来注解，避免运行时解析类型前向引用。
from __future__ import annotations

# 导入命令行解析模块。
import argparse
# 导入 JSON 编解码模块。
import json
# 导入操作系统路径工具。
import os
# 导入外部命令查找工具。
import shutil
# 导入外部进程执行工具。
import subprocess
# 导入高精度计时工具。
import time
# 导入路径类型。
from pathlib import Path
# 导入类型标注工具。
from typing import Any


# 同时兼容包导入和直接脚本执行的场景定义加载。
try:
    # 测试以 scripts 命名空间导入时使用完整模块路径。
    import scripts.usage_build_scenarios as usage_build_scenarios
# 直接执行当前脚本时，脚本目录本身位于模块搜索路径。
except ModuleNotFoundError:
    # 从相邻模块加载同一组场景定义。
    import usage_build_scenarios

# 重导出演示基础 feature 常量，保持既有测试与调用方兼容。
DEMO_BASE_FEATURES = usage_build_scenarios.DEMO_BASE_FEATURES
# 重导出演示日志 feature 常量。
DEMO_LOGGING_FEATURES = usage_build_scenarios.DEMO_LOGGING_FEATURES
# 重导出图形 backend feature 集合，供矩阵测试复核。
GRAPHICS_BACKEND_FEATURES = usage_build_scenarios.GRAPHICS_BACKEND_FEATURES
# 重导出 D3D11 的 windows package feature 集合。
D3D11_WINDOWS_PACKAGE_FEATURES = usage_build_scenarios.D3D11_WINDOWS_PACKAGE_FEATURES
# 重导出 D3D11 专属的 windows package feature 集合。
D3D11_ONLY_WINDOWS_PACKAGE_FEATURES = usage_build_scenarios.D3D11_ONLY_WINDOWS_PACKAGE_FEATURES
# 重导出 D3D12 的 windows package feature 集合。
D3D12_WINDOWS_PACKAGE_FEATURES = usage_build_scenarios.D3D12_WINDOWS_PACKAGE_FEATURES
# 重导出 D3D12 专属的 windows package feature 集合。
D3D12_ONLY_WINDOWS_PACKAGE_FEATURES = usage_build_scenarios.D3D12_ONLY_WINDOWS_PACKAGE_FEATURES
# 重导出全部图形选择面可能启用的 windows package feature 集合。
GRAPHICS_WINDOWS_PACKAGE_FEATURES = usage_build_scenarios.GRAPHICS_WINDOWS_PACKAGE_FEATURES
# 重导出场景数据类型。
Scenario = usage_build_scenarios.Scenario
# 重导出仓库根目录解析函数。
project_root = usage_build_scenarios.project_root
# 重导出完整场景矩阵构造函数。
scenario_specs = usage_build_scenarios.scenario_specs




# 查找 Cargo 可执行文件。
def find_cargo() -> str | None:
    # 使用 PATH 查找，避免假设 Rust 安装目录。
    return shutil.which("cargo")


# 查找 rustc 可执行文件。
def find_rustc() -> str | None:
    # 使用 PATH 查找，便于报告工具链可用性。
    return shutil.which("rustc")


# 从 rustc 详细版本输出中提取 host target triple。
def parse_rustc_host_target(verbose_version: str) -> str | None:
    # 逐行查找 rustc 固定格式的 host 字段。
    for line in verbose_version.splitlines():
        # 只接受明确的 host 前缀，避免误读其他版本字段。
        if line.startswith("host: "):
            # 去掉字段名前缀和两端空白后返回 target triple。
            return line.removeprefix("host: ").strip() or None
    # 缺少 host 字段时返回空值，让调用方拒绝生成不绑定 target 的证据。
    return None


# 查询当前 rustc 的 host target triple。
def rustc_host_target(rustc: str, root: Path) -> str | None:
    # 执行只读版本查询并保留完整输出。
    completed = subprocess.run(
        # 请求包含 host 字段的详细版本格式。
        [rustc, "-vV"],
        # 在仓库根目录执行，保持工具调用上下文一致。
        cwd=root,
        # 手动检查退出码以返回明确空值。
        check=False,
        # 捕获输出供稳定解析。
        capture_output=True,
        # 以文本模式读取 rustc 输出。
        text=True,
        # 固定 UTF-8，避免本地代码页影响字段解析。
        encoding="utf-8",
        # 用替换字符容忍工具链输出中的异常字节。
        errors="replace",
    )
    # rustc 查询失败时不猜测目标平台。
    if completed.returncode != 0:
        # 返回空值交由主入口阻止真实采集。
        return None
    # 解析 rustc 标准详细版本输出。
    return parse_rustc_host_target(completed.stdout)


# 返回 Cargo 为指定 target 读取的 linker 环境变量名。
def target_linker_environment_name(target: str) -> str:
    # 将 target triple 转为 Cargo 约定的全大写下划线形式。
    normalized_target = target.upper().replace("-", "_")
    # 拼接 Cargo 的目标 linker 环境变量名。
    return f"CARGO_TARGET_{normalized_target}_LINKER"


# 执行一条命令并返回可序列化的摘要。
def run_command(command: list[str], cwd: Path, dry_run: bool) -> dict[str, Any]:
    # 生成便于人工复核的命令文本。
    command_text = " ".join(command)
    # dry-run 只展示命令，不产生构建或清理副作用。
    if dry_run:
        # 返回与真实执行兼容的预览结果。
        return {
            "command": command_text,
            "status": "dry-run",
            "returncode": None,
            "duration_seconds": 0.0,
            "_stdout": "",
            "_stderr": "",
        }
    # 记录命令开始时间。
    started = time.perf_counter()
    # 执行命令并保留输出供 metadata 解析。
    completed = subprocess.run(
        command,
        cwd=cwd,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    # 计算命令耗时。
    duration = time.perf_counter() - started
    # 将命令状态归一为通过或失败。
    status = "passed" if completed.returncode == 0 else "failed"
    # 返回公开摘要和内部输出字段。
    return {
        "command": command_text,
        "status": status,
        "returncode": completed.returncode,
        "duration_seconds": round(duration, 3),
        "stdout_tail": completed.stdout[-2000:],
        "stderr_tail": completed.stderr[-2000:],
        "_stdout": completed.stdout,
        "_stderr": completed.stderr,
    }


# 删除命令结果中的内部输出字段。
def public_result(result: dict[str, Any]) -> dict[str, Any]:
    # 只保留报告需要的公开键。
    return {key: value for key, value in result.items() if not key.startswith("_")}


# 返回 compile-fail 输出中缺少的必需错误片段。
def missing_error_fragments(
    result: dict[str, Any], expected_fragments: tuple[str, ...]
) -> list[str]:
    # 合并完整标准输出与标准错误，避免只检查截断摘要。
    output = f"{result.get('_stdout', '')}\n{result.get('_stderr', '')}"
    # 保持声明顺序返回未出现的错误片段。
    return [fragment for fragment in expected_fragments if fragment not in output]


# 返回当前仓库提交，供基线结果绑定源码版本。
def current_commit(root: Path) -> str:
    # 查询当前 HEAD，不修改仓库状态。
    completed = subprocess.run(
        ["git", "rev-parse", "HEAD"],
        cwd=root,
        check=False,
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    # 返回提交哈希或明确的未知标记。
    if completed.returncode == 0:
        # 去掉 Git 输出末尾换行。
        return completed.stdout.strip()
    # Git 查询失败时保留可识别的失败状态。
    return "unknown"


# 返回 Cargo metadata 中与清单对应的 package。
def package_for_manifest(metadata: dict[str, Any], manifest: Path) -> dict[str, Any]:
    # 统一 Windows 路径大小写后进行比较。
    expected = os.path.normcase(str(manifest.resolve()))
    # 遍历 resolved graph 中的 package。
    for package in metadata.get("packages", []):
        # 读取并规范 package 清单路径。
        candidate = os.path.normcase(str(Path(package["manifest_path"]).resolve()))
        # 返回与 fixture 清单完全对应的 package。
        if candidate == expected:
            # 将动态 JSON 对象返回给调用方。
            return package
    # metadata 缺少目标 package 时立即报错。
    raise RuntimeError(f"metadata 中未找到 fixture package: {manifest}")


# 从 package 中提取稳定排序的直接依赖名。
def direct_dependency_names(package: dict[str, Any]) -> list[str]:
    # 读取 Cargo metadata 的直接依赖集合。
    names = {dependency["name"] for dependency in package.get("dependencies", [])}
    # 排序后保证报告具有稳定顺序。
    return sorted(names)


# 返回 Cargo resolve 节点实际包含的 package 名称。
def resolved_package_names(metadata: dict[str, Any]) -> set[str]:
    # 收集当前 feature 与 target 解析结果中的 package 标识。
    resolved_ids = {
        node.get("id")
        for node in metadata.get("resolve", {}).get("nodes", [])
        if node.get("id")
    }
    # 只返回确实出现在 resolve 节点中的 package 名称。
    return {
        package["name"]
        for package in metadata.get("packages", [])
        if package.get("id") in resolved_ids
    }


# 返回指定 package 的全部实际启用 feature。
def resolved_package_features(metadata: dict[str, Any], package_name: str) -> set[str]:
    # 将实际 resolve 节点按 package 标识建立索引。
    resolved_nodes = {
        node["id"]: node
        for node in metadata.get("resolve", {}).get("nodes", [])
        if node.get("id")
    }
    # 找出名称匹配且确实参与当前解析图的 package 标识。
    package_ids = {
        package["id"]
        for package in metadata.get("packages", [])
        if package.get("name") == package_name and package.get("id") in resolved_nodes
    }
    # 初始化多个同名 package 版本的 feature 并集。
    features: set[str] = set()
    # 合并每个匹配 resolve 节点实际选择的 feature。
    for package_id in package_ids:
        # Cargo metadata 的 feature 数组已经使用公开字符串名称。
        features.update(resolved_nodes[package_id].get("features", []))
    # 返回集合供正反 feature 断言使用。
    return features


# 把 package 与 feature 对转换为稳定、可读的报告标签。
def package_feature_labels(assertions: tuple[tuple[str, str], ...]) -> list[str]:
    # 使用 Cargo 常见的 package/feature 形式并保持声明顺序。
    return [f"{package}/{feature}" for package, feature in assertions]


# 返回场景要求但未进入解析图的 package feature 标签。
def missing_required_package_features(
    metadata: dict[str, Any], assertions: tuple[tuple[str, str], ...]
) -> list[str]:
    # 逐 package 读取真实 resolve feature，缺少 package 时同样判定为缺失。
    return sorted(
        f"{package}/{feature}"
        for package, feature in assertions
        if feature not in resolved_package_features(metadata, package)
    )


# 返回场景禁止却进入解析图的 package feature 标签。
def present_forbidden_package_features(
    metadata: dict[str, Any], assertions: tuple[tuple[str, str], ...]
) -> list[str]:
    # 逐 package 读取真实 resolve feature，只报告确实被 Cargo 选择的误入项。
    return sorted(
        f"{package}/{feature}"
        for package, feature in assertions
        if feature in resolved_package_features(metadata, package)
    )


# 从 metadata 中提取可复核的解析依赖图摘要。
def resolved_graph_summary(metadata: dict[str, Any]) -> dict[str, Any]:
    # 将实际 resolve 节点按 package 标识建立索引。
    resolved_nodes = {
        node["id"]: node
        for node in metadata.get("resolve", {}).get("nodes", [])
        if node.get("id")
    }
    # 为每个实际解析的 package 仅保留基线关心的稳定字段。
    packages = [
        {
            "name": package.get("name"),
            "version": package.get("version"),
            "source": package.get("source"),
            "dependencies": sorted(
                dependency.get("name")
                for dependency in resolved_nodes[package["id"]].get("deps", [])
                if dependency.get("name")
            ),
            "features": sorted(resolved_nodes[package["id"]].get("features", [])),
        }
        for package in metadata.get("packages", [])
        if package.get("id") in resolved_nodes
    ]
    # 按名称和版本排序，避免 Cargo 输出顺序影响 diff。
    packages.sort(key=lambda package: (package["name"] or "", package["version"] or ""))
    # 返回 package 数量、resolve 节点数量和完整摘要。
    return {
        "package_count": len(packages),
        "node_count": len(metadata.get("resolve", {}).get("nodes", [])),
        "packages": packages,
    }


# 定位 fixture release 产物并返回大小。
def release_artifact(
    metadata: dict[str, Any], package: dict[str, Any], target: str
) -> dict[str, Any] | None:
    # 读取 Cargo metadata 给出的 target 根目录。
    target_directory = Path(metadata["target_directory"])
    # 仅选择 fixture 自身声明的二进制 target。
    binary_targets = [
        target
        for target in package.get("targets", [])
        if "bin" in target.get("kind", [])
    ]
    # 当前 fixture 应当只有一个 main 二进制 target。
    if not binary_targets:
        # 没有二进制时返回空值而不是猜测文件名。
        return None
    # 取排序后的第一个 target 保持结果稳定。
    target_name = sorted(target["name"] for target in binary_targets)[0]
    # 显式 target 构建把 release 产物放在 target triple 子目录。
    release_directory = target_directory / target / "release"
    # 在 Windows 上 release 二进制带有 exe 后缀。
    candidates = [
        release_directory / f"{target_name}.exe",
        release_directory / target_name,
    ]
    # 返回第一个实际存在的产物信息。
    for candidate in candidates:
        # 检查候选产物是否存在且为文件。
        if candidate.is_file():
            # 返回相对仓库路径和字节大小。
            return {
                "path": str(candidate.resolve().relative_to(project_root().resolve())),
                "bytes": candidate.stat().st_size,
            }
    # dry-run 或构建异常时可能没有产物。
    return None


# 采集单个 fixture 的 clean-build 基线。
def measure_scenario(
    scenario: Scenario,
    root: Path,
    cargo: str,
    host_target: str,
    locked: bool,
    clean_before: bool,
    clean_after: bool,
    dry_run: bool,
) -> dict[str, Any]:
    # 场景未覆盖 target 时使用当前 rustc host triple。
    effective_target = scenario.target or host_target
    # 只把跨宿主的正向 release 识别为必须显式绑定 linker 的场景。
    cross_target_release = (
        # compile-fail 场景只执行 cargo check，不需要最终 linker。
        not scenario.expected_compile_failure
        # check-only 场景不会生成最终可执行文件。
        and not scenario.check_only
        # host 与目标不同才需要记录额外交叉 linker。
        and effective_target != host_target
    # 结束跨目标 release 判定。
    )
    # 计算 Cargo 为当前目标读取的 linker 环境变量名。
    linker_environment = target_linker_environment_name(effective_target)
    # 读取显式 linker 覆盖；空字符串按未配置处理。
    linker_override = os.environ.get(linker_environment)
    # 去掉意外的两端空白，保留可直接执行的 linker 路径。
    linker_override = linker_override.strip() if linker_override else None
    # 初始化 fixture 报告。
    record: dict[str, Any] = {
        "name": scenario.name,
        "description": scenario.description,
        # 记录 feature 参数，确保根二进制场景可复核。
        "feature_args": list(scenario.feature_args),
        # 记录构建目标参数，避免把根清单其他目标误当证据。
        "build_args": list(scenario.build_args),
        # 记录 metadata 过滤与实际构建共同使用的 target triple。
        "target": effective_target,
        # 记录正向场景是否只执行跨目标类型检查。
        "check_only": scenario.check_only,
        "manifest": str(scenario.manifest.resolve().relative_to(root.resolve())),
        "clean_before": clean_before,
        "clean_after": clean_after,
        "status": "failed",
        "steps": [],
        "resolved_graph": None,
        "direct_dependencies": [],
        "dependency_assertions": {
            "required": list(scenario.required_packages),
            "forbidden": list(scenario.forbidden_packages),
            "missing_required": [],
            "present_forbidden": [],
        },
        # 记录既有 package 上精确 Cargo feature 的正反断言。
        "package_feature_assertions": {
            # 保存场景要求启用的 package feature。
            "required": package_feature_labels(scenario.required_package_features),
            # 保存场景要求关闭的 package feature。
            "forbidden": package_feature_labels(scenario.forbidden_package_features),
            # 初始化缺失 package feature 列表。
            "missing_required": [],
            # 初始化误入 package feature 列表。
            "present_forbidden": [],
        },
        # 记录无专属 package capability 的 Cargo feature 正反断言。
        "uix_feature_assertions": {
            # 保存场景要求启用的 uix feature。
            "required": list(scenario.required_uix_features),
            # 保存场景要求关闭的 uix feature。
            "forbidden": list(scenario.forbidden_uix_features),
            # 初始化缺失 feature 列表。
            "missing_required": [],
            # 初始化误入 feature 列表。
            "present_forbidden": [],
        },
        "expected_outcome": (
            # 禁用公开面场景必须以匹配诊断的失败结束。
            "compile-fail"
            if scenario.expected_compile_failure
            # 跨目标正向场景只要求类型检查成功。
            else ("check-success" if scenario.check_only else "build-success")
        ),
        "compile_fail_assertions": {
            "required_fragments": list(scenario.expected_error_fragments),
            "missing_fragments": [],
        },
        # 跨目标 release 必须记录显式 linker 与版本探针，避免遗漏关键工具链。
        "cross_target_linker": (
            # 只有真正跨宿主链接的场景需要该证据对象。
            {
                # 保存 Cargo 读取的环境变量名，便于复现实验。
                "environment": linker_environment,
                # 保存本轮实际传给 Cargo 的 linker 覆盖。
                "override": linker_override,
                # 版本探针在执行阶段填充。
                "probe": None,
            }
            # host release 与 check 场景不虚构额外 linker 证据。
            if cross_target_release
            else None
        ),
        "release_artifact": None,
    }
    # 预先准备所有 Cargo 子命令共用的清单参数。
    manifest_args = ["--manifest-path", str(scenario.manifest)]
    # 显式绑定编译目标，避免默认 host 漂移后沿用旧证据。
    target_args = ["--target", effective_target]
    # metadata 必须过滤到同一目标，不能混入其他 target 条件依赖。
    metadata_target_args = ["--filter-platform", effective_target]
    # 追加 locked 参数，确保清单锁文件参与解析。
    locked_suffix = ["--locked"] if locked else []
    # 无论前置步骤是否失败，都尝试执行收尾清理。
    try:
        # 默认先清理 fixture 的构建目录。
        if clean_before:
            # 执行 cargo clean，避免复用上一个 fixture 的 target。
            clean_result = run_command([cargo, "clean", *manifest_args], root, dry_run)
            # 记录前置清理结果。
            record["steps"].append({"name": "clean-before", **public_result(clean_result)})
            # 真实清理失败时停止当前 fixture。
            if clean_result["status"] == "failed":
                # 将命令错误交给统一异常记录逻辑。
                raise RuntimeError("cargo clean（前置）失败")
        # 跨目标 release 必须先证明显式 linker 可执行并记录其版本。
        if cross_target_release:
            # 真实采集缺少 linker 覆盖时立即失败，不浪费时间等待最终链接报错。
            if linker_override is None and not dry_run:
                # 给出 Cargo 标准环境变量名，便于调用方补齐工具链。
                raise RuntimeError(f"跨目标 release 缺少 linker：请设置 {linker_environment}")
            # 已配置 linker 时执行只读版本探针并写入结构化证据。
            if linker_override is not None:
                # 调用 linker 的标准版本参数，不创建构建产物。
                linker_probe = run_command([linker_override, "--version"], root, dry_run)
                # 保存探针摘要到跨目标 linker 证据对象。
                record["cross_target_linker"]["probe"] = public_result(linker_probe)
                # 同时加入步骤列表，保留统一的执行顺序与状态审计。
                record["steps"].append(
                    # 标记该步骤只负责 linker 版本探针。
                    {"name": "linker-probe", **public_result(linker_probe)}
                # 结束 linker 探针步骤记录。
                )
                # linker 探针失败时禁止继续生成 release 证据。
                if linker_probe["status"] == "failed":
                    # 返回明确错误，避免后续 Cargo 失败掩盖工具不可执行。
                    raise RuntimeError("跨目标 release linker 版本探针失败")
        # 解析完整依赖图和锁定 package 信息。
        # 组装绑定 target 过滤与 feature 集的 metadata 命令。
        metadata_command = [cargo, "metadata", *manifest_args, "--format-version", "1", *metadata_target_args, *scenario.feature_args, *locked_suffix]
        # 执行目标感知的依赖图解析。
        metadata_result = run_command(
            metadata_command,
            root,
            dry_run,
        )
        # 记录 metadata 命令结果。
        record["steps"].append({"name": "metadata", **public_result(metadata_result)})
        # 真实 metadata 失败时停止当前 fixture。
        if metadata_result["status"] == "failed":
            # 保留失败步骤而不继续构建。
            raise RuntimeError("cargo metadata 失败")
        # dry-run 没有真实 JSON，可明确标记图为空。
        if dry_run:
            # 预览模式只报告命令，不虚构依赖图或产物。
            record["resolved_graph"] = {"status": "dry-run"}
        else:
            # 解析 Cargo metadata 的完整标准输出。
            metadata = json.loads(metadata_result["_stdout"])
            # 定位 fixture 自身 package。
            package = package_for_manifest(metadata, scenario.manifest)
            # 记录直接依赖名，作为 capability 变化的快速断言。
            record["direct_dependencies"] = direct_dependency_names(package)
            # 记录完整 resolved graph 摘要。
            record["resolved_graph"] = resolved_graph_summary(metadata)
            # 提取当前场景实际解析到的 package 名称。
            resolved_packages = resolved_package_names(metadata)
            # 找出场景声明但未进入解析图的必需 package。
            missing_required = sorted(set(scenario.required_packages) - resolved_packages)
            # 找出未选择能力却意外进入解析图的禁用 package。
            present_forbidden = sorted(set(scenario.forbidden_packages) & resolved_packages)
            # 把依赖存在性断言写入结构化报告。
            record["dependency_assertions"] = {
                "required": list(scenario.required_packages),
                "forbidden": list(scenario.forbidden_packages),
                "missing_required": missing_required,
                "present_forbidden": present_forbidden,
            }
            # 依赖断言失败时禁止继续生成可误读的通过报告。
            if missing_required or present_forbidden:
                # 返回同时包含缺失与误入依赖的明确错误。
                raise RuntimeError(
                    f"依赖图断言失败：缺少 {missing_required}，意外出现 {present_forbidden}"
                )
            # 找出场景声明但未被 Cargo 选择的必需 package feature。
            missing_required_package_feature_labels = missing_required_package_features(
                metadata, scenario.required_package_features
            )
            # 找出场景要求关闭却意外被 Cargo 选择的 package feature。
            present_forbidden_package_feature_labels = present_forbidden_package_features(
                metadata, scenario.forbidden_package_features
            )
            # 把 package feature 断言写入结构化报告。
            record["package_feature_assertions"] = {
                # 保存要求启用的 package feature。
                "required": package_feature_labels(scenario.required_package_features),
                # 保存要求关闭的 package feature。
                "forbidden": package_feature_labels(scenario.forbidden_package_features),
                # 保存实际缺失的 package feature。
                "missing_required": missing_required_package_feature_labels,
                # 保存实际误入的 package feature。
                "present_forbidden": present_forbidden_package_feature_labels,
            }
            # package feature 断言失败时禁止继续生成可误读的通过报告。
            if (
                missing_required_package_feature_labels
                or present_forbidden_package_feature_labels
            ):
                # 返回同时包含缺失与误入 package feature 的明确错误。
                raise RuntimeError(
                    "package feature 断言失败："
                    f"缺少 {missing_required_package_feature_labels}，"
                    f"意外出现 {present_forbidden_package_feature_labels}"
                )
            # 提取 uix package 在当前场景中实际启用的 feature。
            resolved_uix_features = resolved_package_features(metadata, "uix")
            # 找出场景声明但未启用的必需 feature。
            missing_required_features = sorted(
                set(scenario.required_uix_features) - resolved_uix_features
            )
            # 找出场景要求关闭却意外启用的 feature。
            present_forbidden_features = sorted(
                set(scenario.forbidden_uix_features) & resolved_uix_features
            )
            # 把 uix feature 断言写入结构化报告。
            record["uix_feature_assertions"] = {
                # 保存要求启用的 feature。
                "required": list(scenario.required_uix_features),
                # 保存要求关闭的 feature。
                "forbidden": list(scenario.forbidden_uix_features),
                # 保存实际缺失的 feature。
                "missing_required": missing_required_features,
                # 保存实际误入的 feature。
                "present_forbidden": present_forbidden_features,
            }
            # feature 断言失败时禁止继续生成可误读的通过报告。
            if missing_required_features or present_forbidden_features:
                # 返回同时包含缺失与误入 feature 的明确错误。
                raise RuntimeError(
                    "uix feature 断言失败："
                    f"缺少 {missing_required_features}，意外出现 {present_forbidden_features}"
                )
        # compile-fail 场景验证禁用能力的公开入口确实不可用。
        if scenario.expected_compile_failure:
            # 执行使用方编译检查并保留完整诊断。
            # 组装绑定编译 target、feature 与目标选择的检查命令。
            check_command = [cargo, "check", *manifest_args, *target_args, *scenario.feature_args, *scenario.build_args, *locked_suffix]
            # 执行目标感知的 compile-fail 检查。
            check_result = run_command(
                check_command,
                root,
                dry_run,
            )
            # dry-run 不虚构错误片段，真实运行才检查诊断内容。
            missing_fragments = (
                []
                if dry_run
                else missing_error_fragments(check_result, scenario.expected_error_fragments)
            )
            # 复制公开命令结果，随后按预期失败语义归一状态。
            check_record = public_result(check_result)
            # 真实非零退出且错误片段完整时，compile-fail 步骤才算通过。
            compile_fail_passed = dry_run or (
                check_result["returncode"] not in (None, 0) and not missing_fragments
            )
            # 非 dry-run 时把预期的命令失败转成门禁通过状态。
            if not dry_run:
                # 报告状态表达门禁结论，不沿用子进程的原始失败标签。
                check_record["status"] = "passed" if compile_fail_passed else "failed"
            # 记录 compile-fail 命令及归一后的门禁状态。
            record["steps"].append({"name": "compile-fail-check", **check_record})
            # 把错误片段断言写入结构化报告。
            record["compile_fail_assertions"] = {
                "required_fragments": list(scenario.expected_error_fragments),
                "missing_fragments": missing_fragments,
            }
            # 编译意外成功意味着禁用能力仍从公开面泄漏。
            if not dry_run and check_result["returncode"] == 0:
                # 阻止把公开面泄漏记录为通过。
                raise RuntimeError("compile-fail fixture 意外编译成功")
            # 错误片段不完整意味着失败原因不符合门禁目标。
            if missing_fragments:
                # 明确列出缺少的诊断片段供修复。
                raise RuntimeError(f"compile-fail 诊断缺少片段: {missing_fragments}")
        # 正向跨目标场景执行 cargo check，避免把缺失目标链接器误判为源码失败。
        elif scenario.check_only:
            # 组装绑定编译 target、feature 与目标选择的正向检查命令。
            check_command = [cargo, "check", *manifest_args, *target_args, *scenario.feature_args, *scenario.build_args, *locked_suffix]
            # 执行目标感知的正向类型检查。
            check_result = run_command(
                check_command,
                root,
                dry_run,
            )
            # 记录正向检查命令及其状态。
            record["steps"].append({"name": "check", **public_result(check_result)})
            # 真实类型检查失败时阻止场景通过。
            if check_result["status"] == "failed":
                # 让统一异常处理记录清晰原因。
                raise RuntimeError("cargo check 失败")
        else:
            # 记录正向场景的 release 构建开始时间。
            # 组装绑定编译 target、feature 与二进制目标的 release 命令。
            build_command = [cargo, "build", *manifest_args, "--release", *target_args, *scenario.feature_args, *scenario.build_args, *locked_suffix]
            # 执行目标感知的 clean release 构建。
            build_result = run_command(
                build_command,
                root,
                dry_run,
            )
            # 记录 release 构建命令结果和耗时。
            record["steps"].append({"name": "build-release", **public_result(build_result)})
            # 真实构建失败时将 fixture 标记为失败。
            if build_result["status"] == "failed":
                # 让统一异常处理记录清晰原因。
                raise RuntimeError("cargo build --release 失败")
            # 真实构建成功后读取 release 产物大小。
            if not dry_run:
                # metadata 在非 dry-run 分支中已经完成初始化。
                record["release_artifact"] = release_artifact(metadata, package, effective_target)
        # 只有所有步骤通过后才标记 fixture 成功。
        record["status"] = "dry-run" if dry_run else "passed"
    except (OSError, RuntimeError, json.JSONDecodeError) as error:
        # 把异常写入报告，便于后续修复工具链或 fixture。
        record["error"] = str(error)
    finally:
        # 默认在每个 fixture 完成后清理构建产物。
        if clean_after:
            # 执行 cargo clean，确保报告采集结束后没有 target 残留。
            clean_result = run_command([cargo, "clean", *manifest_args], root, dry_run)
            # 记录后置清理结果。
            record["steps"].append({"name": "clean-after", **public_result(clean_result)})
            # 清理失败时覆盖成功状态并记录原因。
            if clean_result["status"] == "failed":
                # 统一向报告标注收尾清理失败。
                record["status"] = "failed"
                # 追加清理错误而不掩盖前面已有错误。
                record.setdefault("error", "cargo clean（后置）失败")
    # 返回单个 fixture 的最终报告。
    return record


# 创建命令行参数解析器。
def argument_parser(root: Path) -> argparse.ArgumentParser:
    # 初始化脚本参数解析器。
    parser = argparse.ArgumentParser(description=__doc__)
    # 允许只采集一个 fixture，便于增量验证。
    parser.add_argument("--scenario", choices=[scenario.name for scenario in scenario_specs(root)])
    # 允许显式指定 JSON 证据输出路径。
    parser.add_argument(
        "--output",
        type=Path,
        default=root / "test-reports" / "usage-build-baseline.json",
    )
    # 要求 Cargo 使用 Cargo.lock 的精确解析结果。
    parser.add_argument("--locked", action="store_true")
    # 只打印命令，不执行 Rust 构建或清理。
    parser.add_argument("--dry-run", action="store_true")
    # 允许主人显式保留构建产物用于人工检查。
    parser.add_argument("--keep-build-artifacts", action="store_true")
    # 允许预览或调试时跳过前置清理，但默认仍然清理。
    parser.add_argument("--no-clean", action="store_true")
    # 返回完整配置的解析器。
    return parser


# 执行基线采集入口。
def main() -> int:
    # 定位仓库根目录。
    root = project_root()
    # 解析命令行参数。
    args = argument_parser(root).parse_args()
    # 查找 Cargo 和 rustc，供真实运行报告工具链。
    cargo = find_cargo()
    rustc = find_rustc()
    # 没有 Cargo 或 rustc 时只允许 dry-run，避免产生不绑定 target 的证据。
    if (cargo is None or rustc is None) and not args.dry_run:
        # 将工具链缺失明确报告给调用方。
        print("Cargo 或 rustc 不在 PATH；请使用 --dry-run 或先安装 Rust 工具链。")
        # 返回非零状态，阻止把未完成采集当作成功。
        return 2
    # dry-run 使用占位命令名来展示最终命令。
    cargo_command = cargo or "cargo"
    # 工具链可用时读取真实 host target，dry-run 缺少 rustc 时使用可识别占位符。
    host_target = rustc_host_target(rustc, root) if rustc is not None else "host-target"
    # 真实采集无法解析 host 时拒绝继续，避免 target 事实缺失。
    if host_target is None:
        # 输出明确错误供修复工具链或版本解析。
        print("无法从 rustc -vV 解析 host target；停止生成构建证据。")
        # 返回非零状态阻止写入不完整报告。
        return 2
    # 计算本轮是否在 fixture 前后清理 target。
    clean_before = not args.no_clean
    # 只有显式保留参数才跳过后置清理。
    clean_after = not args.keep_build_artifacts
    # 选择全部 fixture 或命令行指定的单个 fixture。
    scenarios = scenario_specs(root)
    # 根据名称过滤单 fixture 采集。
    if args.scenario:
        # 只保留用户指定的 fixture。
        scenarios = [scenario for scenario in scenarios if scenario.name == args.scenario]
    # 初始化总报告并绑定源码提交。
    report: dict[str, Any] = {
        # schema v6 增加跨目标 release linker 覆盖与版本探针证据。
        "schema_version": 6,
        "commit": current_commit(root),
        "locked": args.locked,
        "dry_run": args.dry_run,
        "clean_before": clean_before,
        "clean_after": clean_after,
        "cargo": cargo,
        "rustc": rustc,
        # 记录本轮所有默认场景使用的 rustc host target。
        "host_target": host_target,
        "scenarios": [],
    }
    # 逐个 fixture 采集，保证每个入口拥有独立的清理边界。
    for scenario in scenarios:
        # 追加当前 fixture 报告。
        report["scenarios"].append(
            measure_scenario(
                scenario,
                root,
                cargo_command,
                host_target,
                args.locked,
                clean_before,
                clean_after,
                args.dry_run,
            )
        )
    # dry-run 只输出预览结果，不写入证据文件。
    if args.dry_run:
        # 使用中文缩进输出，方便没有 Rust 工具链时核对命令。
        print(json.dumps(report, ensure_ascii=False, indent=2))
        # 预览命令生成成功即可返回通过。
        return 0
    # 创建证据目录，真实采集结果写入 test-reports。
    args.output.parent.mkdir(parents=True, exist_ok=True)
    # 写入稳定、可审阅的 JSON 证据文件。
    args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    # 只有所有 fixture 均成功且清理完成才返回通过。
    return 0 if all(scenario["status"] == "passed" for scenario in report["scenarios"]) else 1


# 仅在脚本直接运行时进入主函数。
if __name__ == "__main__":
    # 将主函数状态码传递给操作系统。
    raise SystemExit(main())
