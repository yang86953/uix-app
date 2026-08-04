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
# 导入数据类工具。
from dataclasses import dataclass
# 导入路径类型。
from pathlib import Path
# 导入类型标注工具。
from typing import Any


# 描述一个独立的使用方基线入口。
@dataclass(frozen=True)
class Scenario:
    # 记录报告中使用的稳定名称。
    name: str
    # 记录 fixture 的 Cargo 清单路径。
    manifest: Path
    # 记录该入口覆盖的能力范围。
    description: str
    # 记录 resolved graph 中必须出现的专属 package。
    required_packages: tuple[str, ...] = ()
    # 记录 resolved graph 中必须缺席的未选 package。
    forbidden_packages: tuple[str, ...] = ()
    # 标记该场景是否必须在公开入口编译阶段失败。
    expected_compile_failure: bool = False
    # 记录 compile-fail 场景必须出现的错误片段。
    expected_error_fragments: tuple[str, ...] = ()


# 返回仓库根目录。
def project_root() -> Path:
    # 当前脚本位于仓库根目录下的 scripts 目录。
    return Path(__file__).resolve().parents[1]


# 返回 ODC-01/ODC-07 的独立 fixture 定义。
def scenario_specs(root: Path) -> list[Scenario]:
    # 返回最小入口、默认入口、单能力入口与禁用公开面入口。
    return [
        # 最小入口关闭所有默认 feature。
        Scenario(
            name="minimal",
            manifest=root / "fixtures" / "usage-build" / "minimal" / "Cargo.toml",
            description="关闭默认 feature 的最小使用方入口",
            # 最小入口必须排除全部已独立裁剪的专属依赖。
            forbidden_packages=("image", "qrcode", "regex"),
        ),
        # 默认入口覆盖当前 Windows 默认 D3D11 能力。
        Scenario(
            name="d3d11-default",
            manifest=root / "fixtures" / "usage-build" / "d3d11-default" / "Cargo.toml",
            description="使用当前默认 feature 的图形入口",
            # 默认兼容集合必须包含图片、二维码与表单正则依赖。
            required_packages=("image", "qrcode", "regex"),
        ),
        # 单能力入口只打开设置序列化能力。
        Scenario(
            name="settings-serde",
            manifest=root / "fixtures" / "usage-build" / "settings-serde" / "Cargo.toml",
            description="只打开 settings-serde capability 的入口",
            # 设置序列化入口不得合并图片、二维码或表单正则依赖。
            forbidden_packages=("image", "qrcode", "regex"),
        ),
        # 图片编解码单能力入口只打开对应文件格式 capability。
        # 创建图片编解码正向使用方场景。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="image-codecs",
            # 指向图片编解码正向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "image-codecs" / "Cargo.toml",
            # 说明该入口只覆盖图片编解码能力。
            description="只打开 image-codecs capability 的入口",
            # 图片编解码入口必须解析精确 image package。
            required_packages=("image",),
            # 图片编解码入口不得合并其他独立 capability 依赖。
            forbidden_packages=("qrcode", "regex"),
        # 结束图片编解码正向场景定义。
        ),
        # 二维码单能力入口只打开对应组件 capability。
        Scenario(
            name="qrcode",
            manifest=root / "fixtures" / "usage-build" / "qrcode" / "Cargo.toml",
            description="只打开 qrcode capability 的入口",
            required_packages=("qrcode",),
            # 二维码入口不得合并图片或表单正则依赖。
            forbidden_packages=("image", "regex"),
        ),
        # 表单 pattern 单能力入口只打开正则规则 capability。
        Scenario(
            name="form-pattern",
            manifest=root / "fixtures" / "usage-build" / "form-pattern" / "Cargo.toml",
            description="只打开 form-pattern capability 的入口",
            required_packages=("regex",),
            # 表单正则入口不得合并图片或二维码依赖。
            forbidden_packages=("image", "qrcode"),
        ),
        # 二维码禁用入口必须证明公开类型无法绕过 capability。
        Scenario(
            name="qrcode-disabled",
            manifest=root / "fixtures" / "usage-build" / "qrcode-disabled" / "Cargo.toml",
            description="关闭 qrcode capability 的公开入口 compile-fail",
            # 禁用入口必须排除全部已独立裁剪的专属依赖。
            forbidden_packages=("image", "qrcode", "regex"),
            expected_compile_failure=True,
            expected_error_fragments=("unresolved import", "QRCode"),
        ),
        # 表单 pattern 禁用入口必须证明 builder 方法无法绕过 capability。
        Scenario(
            name="form-pattern-disabled",
            manifest=root / "fixtures" / "usage-build" / "form-pattern-disabled" / "Cargo.toml",
            description="关闭 form-pattern capability 的公开方法 compile-fail",
            # 禁用入口必须排除全部已独立裁剪的专属依赖。
            forbidden_packages=("image", "qrcode", "regex"),
            expected_compile_failure=True,
            expected_error_fragments=("no method named", "validate_pattern"),
        ),
        # 图片编解码禁用入口必须证明公开方法无法绕过 capability。
        # 创建图片编解码负向使用方场景。
        Scenario(
            # 记录报告中的稳定场景名称。
            name="image-codecs-disabled",
            # 指向图片编解码负向 fixture 清单。
            manifest=root / "fixtures" / "usage-build" / "image-codecs-disabled" / "Cargo.toml",
            # 说明该入口必须在公开方法编译阶段失败。
            description="关闭 image-codecs capability 的公开方法 compile-fail",
            # 禁用入口必须排除全部已独立裁剪的专属依赖。
            forbidden_packages=("image", "qrcode", "regex"),
            # 标记该场景预期编译失败。
            expected_compile_failure=True,
            # 绑定稳定的缺失方法诊断片段。
            expected_error_fragments=("no method named", "load_from_bytes"),
        # 结束图片编解码负向场景定义。
        ),
    ]


# 查找 Cargo 可执行文件。
def find_cargo() -> str | None:
    # 使用 PATH 查找，避免假设 Rust 安装目录。
    return shutil.which("cargo")


# 查找 rustc 可执行文件。
def find_rustc() -> str | None:
    # 使用 PATH 查找，便于报告工具链可用性。
    return shutil.which("rustc")


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
def release_artifact(metadata: dict[str, Any], package: dict[str, Any]) -> dict[str, Any] | None:
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
    # 在 Windows 上 release 二进制带有 exe 后缀。
    candidates = [
        target_directory / "release" / f"{target_name}.exe",
        target_directory / "release" / target_name,
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
    locked: bool,
    clean_before: bool,
    clean_after: bool,
    dry_run: bool,
) -> dict[str, Any]:
    # 初始化 fixture 报告。
    record: dict[str, Any] = {
        "name": scenario.name,
        "description": scenario.description,
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
        "expected_outcome": (
            "compile-fail" if scenario.expected_compile_failure else "build-success"
        ),
        "compile_fail_assertions": {
            "required_fragments": list(scenario.expected_error_fragments),
            "missing_fragments": [],
        },
        "release_artifact": None,
    }
    # 预先准备所有 Cargo 子命令共用的清单参数。
    manifest_args = ["--manifest-path", str(scenario.manifest)]
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
        # 解析完整依赖图和锁定 package 信息。
        metadata_result = run_command(
            [cargo, "metadata", *manifest_args, "--format-version", "1"] + locked_suffix,
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
        # compile-fail 场景验证禁用能力的公开入口确实不可用。
        if scenario.expected_compile_failure:
            # 执行使用方编译检查并保留完整诊断。
            check_result = run_command(
                [cargo, "check", *manifest_args] + locked_suffix,
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
        else:
            # 记录正向场景的 release 构建开始时间。
            build_result = run_command(
                [cargo, "build", *manifest_args, "--release"] + locked_suffix,
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
                record["release_artifact"] = release_artifact(metadata, package)
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
    # 没有 Cargo 时只允许 dry-run，避免产生误导性证据。
    if cargo is None and not args.dry_run:
        # 将工具链缺失明确报告给调用方。
        print("Cargo 不在 PATH；请使用 --dry-run 或先安装 Rust 工具链。")
        # 返回非零状态，阻止把未完成采集当作成功。
        return 2
    # dry-run 使用占位命令名来展示最终命令。
    cargo_command = cargo or "cargo"
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
        "schema_version": 1,
        "commit": current_commit(root),
        "locked": args.locked,
        "dry_run": args.dry_run,
        "clean_before": clean_before,
        "clean_after": clean_after,
        "cargo": cargo,
        "rustc": rustc,
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
