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
# 列出不得持有时点状态副本的稳定事实入口。
TIMEPOINT_STATUS_FILES = (
    # 产品索引只声明稳定产品边界。
    ROOT / "docs" / "产品.md",
    # 能力正文只声明能做、不做与暂缓项。
    ROOT / "docs" / "产品" / "能力.md",
    # 使用入口只声明稳定入口与权威路由。
    ROOT / "docs" / "使用" / "入门" / "入口与阅读路径.md",
    # 结束时点状态文件集合。
)
# 列出不得重新复制到稳定事实入口的时点状态标记。
FORBIDDEN_TIMEPOINT_STATUS_MARKERS = (
    # 禁止产品入口保存当前阻塞标题。
    "当前阻塞",
    # 禁止使用入口保存当前发布状态块。
    "> **当前状态**",
    # 结束禁止标记集合。
)
# 定位只负责稳定入口与权威路由的使用文档。
USAGE_ENTRY_FILE = ROOT / "docs" / "使用" / "入门" / "入口与阅读路径.md"
# 定义项目动态状态的唯一跟踪入口。
GITEA_PARENT_ISSUE_ROUTE = "http://100.79.245.29:3000/admin/uix-app/issues/1"
# 列出使用入口必须保留的权威状态路由。
USAGE_ENTRY_REQUIRED_ROUTES = (
    # 版本、发布与交付事实归属产品交付文档。
    "../../产品/交付与许可.md",
    # 进行中任务、测试覆盖与环境差距归属 Gitea 父 Issue。
    GITEA_PARENT_ISSUE_ROUTE,
    # 结束权威路由集合。
)
# 定位只保存稳定交付契约的产品文档。
DELIVERY_CONTRACT_FILE = ROOT / "docs" / "产品" / "交付与许可.md"
# 列出不得复制到稳定交付契约的发布进展标记。
FORBIDDEN_DELIVERY_PROGRESS_MARKERS = (
    # 禁止把整个交付章节声明为当前进展快照。
    "## 当前交付状态",
    # 禁止复制带时点的主要测试环境。
    "当前主要测试环境",
    # 禁止复制带时点的候选包验证结论。
    "当前已验证的 Windows",
    # 禁止复制正式发布进展结论。
    "尚未正式发布",
    # 结束发布进展标记集合。
)
# 列出只应陈述公开行为并路由环境覆盖的使用文档。
USAGE_COVERAGE_ROUTING_FILES = (
    # 图形配置只陈述后端选择与失败契约。
    ROOT / "docs" / "使用" / "框架设施" / "配置.md",
    # 运行保障只陈述诊断与恢复公开契约。
    ROOT / "docs" / "使用" / "框架设施" / "运行保障.md",
    # 结束环境覆盖路由文件集合。
)
# 列出不得复制到使用文档的实现或环境覆盖进展标记。
FORBIDDEN_USAGE_COVERAGE_MARKERS = (
    # 禁止保留当前实现进展章节。
    "## 当前实现状态",
    # 禁止保留当前完成进展前缀。
    "当前已完成 callback",
    # 禁止复制图形真机覆盖结论。
    "真机测试尚未覆盖",
    # 禁止复制平台 teardown 覆盖结论。
    "尚未由当前测试覆盖",
    # 结束使用文档进展标记集合。
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
    # 确认稳定事实入口把当前项目状态留在 Gitea。
    def test_current_project_status_stays_in_gitea(self) -> None:
        # 收集仍然写入时点状态副本的稳定入口。
        violations: list[str] = []
        # 遍历精确的稳定事实入口。
        for path in TIMEPOINT_STATUS_FILES:
            # 读取当前文档文本。
            content = path.read_text(encoding="utf-8")
            # 检查每个禁止的时点状态标记。
            for marker in FORBIDDEN_TIMEPOINT_STATUS_MARKERS:
                # 只记录仍然持有禁止标记的文件。
                if marker in content:
                    # 保存便于定位的仓库相对路径与标记。
                    violations.append(f"{path.relative_to(ROOT).as_posix()}: {marker}")
        # 时点状态必须只由 Gitea 父 Issue #1 持有。
        self.assertEqual(violations, [])

    # 确认使用入口把动态状态路由到各自权威来源。
    def test_usage_entry_routes_status_to_authorities(self) -> None:
        # 读取使用入口当前文本。
        content = USAGE_ENTRY_FILE.read_text(encoding="utf-8")
        # 收集缺失的权威状态路由。
        missing = [route for route in USAGE_ENTRY_REQUIRED_ROUTES if route not in content]
        # 使用入口必须同时指向产品交付文档和 Gitea。
        self.assertEqual(missing, [])

    # 确认交付文档只保存稳定契约并把发布进展路由到 Gitea。
    def test_delivery_contract_routes_progress_to_gitea(self) -> None:
        # 读取产品交付契约当前文本。
        content = DELIVERY_CONTRACT_FILE.read_text(encoding="utf-8")
        # 收集仍然复制到交付契约的发布进展标记。
        violations = [marker for marker in FORBIDDEN_DELIVERY_PROGRESS_MARKERS if marker in content]
        # 稳定交付契约不得保留发布进展副本。
        self.assertEqual(violations, [])
        # 交付契约必须路由到项目跟踪入口。
        self.assertIn(GITEA_PARENT_ISSUE_ROUTE, content)
        # 交付契约必须明确发布父任务编号。
        self.assertIn("#1", content)

    # 确认使用文档只陈述公开行为并把环境覆盖路由到 Gitea。
    def test_usage_docs_route_environment_coverage_to_gitea(self) -> None:
        # 收集仍然复制到使用文档的环境覆盖进展。
        violations: list[str] = []
        # 收集没有指向项目跟踪入口的使用文档。
        missing_routes: list[str] = []
        # 遍历精确的使用文档集合。
        for path in USAGE_COVERAGE_ROUTING_FILES:
            # 读取当前使用文档文本。
            content = path.read_text(encoding="utf-8")
            # 检查每个禁止的进展标记。
            for marker in FORBIDDEN_USAGE_COVERAGE_MARKERS:
                # 只记录仍然持有禁止标记的文档。
                if marker in content:
                    # 保存便于定位的仓库相对路径与标记。
                    violations.append(f"{path.relative_to(ROOT).as_posix()}: {marker}")
            # 检查使用文档是否保留项目跟踪路由。
            if GITEA_PARENT_ISSUE_ROUTE not in content:
                # 保存缺失路由的仓库相对路径。
                missing_routes.append(path.relative_to(ROOT).as_posix())
        # 使用文档不得保存环境覆盖进展副本。
        self.assertEqual(violations, [])
        # 使用文档必须把环境覆盖路由到 Gitea。
        self.assertEqual(missing_routes, [])

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
