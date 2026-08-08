# 声明测试文件使用 UTF-8 编码。
# -*- coding: utf-8 -*-
# 说明本测试守护只以测试判断项目完成的仓库约定。
"""Guard the repository policy that completion relies on tests only."""

# 启用延迟解析类型标注。
from __future__ import annotations

# 引入标准库单元测试框架。
import unittest
# 引入跨平台路径类型。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 收集所有权威 Markdown 文档与入口说明。
DOCUMENTATION_FILES = tuple(sorted((ROOT / "docs").rglob("*.md"))) + (
    # 纳入仓库首页。
    ROOT / "README.md",
    # 纳入版本变更记录。
    ROOT / "CHANGELOG.md",
    # 纳入演示工作区说明。
    ROOT / "demo" / "README.md",
    # 结束文档入口集合。
)
# 列出不得作为项目完成入口出现的非测试命令。
FORBIDDEN_COMMANDS = ("cargo check", "cargo clippy", "cargo bench", "cargo fmt -- --check")
# 列出已经取消的独立验证路径。
FORBIDDEN_PATHS = (
    # 禁止恢复独立基准目录。
    ROOT / "benchmarks",
    # 禁止恢复使用方构建矩阵 fixture。
    ROOT / "fixtures" / "usage-build",
    # 禁止恢复独立结果输出目录。
    ROOT / "test-reports",
    # 禁止恢复运行时性能采集探针。
    ROOT / "src" / "core" / "perf_probe.rs",
    # 禁止恢复 G5 发布采集器。
    ROOT / "scripts" / "run_release_g5.py",
    # 禁止恢复 Windows 图形采集器。
    ROOT / "scripts" / "run_windows_gfx_r5.py",
    # 禁止恢复使用方构建测量器。
    ROOT / "scripts" / "measure_usage_build.py",
    # 结束已取消路径集合。
)


# 定义仅测试完成口径的回归测试。
class ValidationPolicyTests(unittest.TestCase):
    # 确认用户文档不再宣传非测试验证命令。
    def test_documentation_only_advertises_tests_for_completion(self) -> None:
        # 收集命令与文档路径的冲突。
        violations: list[str] = []
        # 遍历所有权威文档入口。
        for path in DOCUMENTATION_FILES:
            # 读取当前文档文本。
            content = path.read_text(encoding="utf-8")
            # 检查每条已取消命令。
            for command in FORBIDDEN_COMMANDS:
                # 只记录实际出现的命令。
                if command in content:
                    # 保存便于定位的仓库相对路径与命令。
                    violations.append(f"{path.relative_to(ROOT).as_posix()}: {command}")
        # 所有用户文档都必须只指向测试命令。
        self.assertEqual(violations, [])

    # 确认独立非测试验证入口不会被重新加入仓库。
    def test_standalone_non_test_validation_paths_stay_removed(self) -> None:
        # 收集仍然存在的已取消路径。
        existing = [path.relative_to(ROOT).as_posix() for path in FORBIDDEN_PATHS if path.exists()]
        # 已取消路径必须全部保持不存在。
        self.assertEqual(existing, [])
