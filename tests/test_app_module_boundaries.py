# 声明测试文件使用 UTF-8 编码。
# -*- coding: utf-8 -*-
# 说明本测试守护 app System 内的 Agent 跨 Module 契约归属。
"""Guard app module isolation around the System-private Agent contracts."""

# 启用延迟解析类型标注。
from __future__ import annotations

# 引入正则表达式以识别 Rust 模块路径依赖。
import re
# 引入标准库单元测试框架。
import unittest
# 引入跨平台路径类型。
from pathlib import Path


# 定位仓库根目录。
ROOT = Path(__file__).resolve().parents[1]
# 定位 app System 源码根目录。
APP_ROOT = ROOT / "src" / "app"
# 列出不得直接依赖 agent Module 的目录边界。
GUARDED_DIRECTORIES = (
    # event-loop 只消费 window 与 System 私有契约。
    APP_ROOT / "event_loop",
    # window 只消费 System 私有命令和语义契约。
    APP_ROOT / "window",
    # queues 定义契约，不能反向依赖 agent 的实现。
    APP_ROOT / "queues",
)
# 列出同样不得反向依赖 agent 实现的单文件边界。
GUARDED_FILES = (
    # 语义发布端口由 System 私有边界拥有。
    APP_ROOT / "window_semantics.rs",
)
# 识别绝对、花括号与相对形式的 app::agent 路径。
FORBIDDEN_AGENT_REFERENCE = re.compile(
    # 匹配去除空白后的 Rust 路径，不依赖具体 use 排版。
    r"(?:crate::app::|crate::app::\{(?:self::)?|super(?::super)*::)agent(?:as|::|,|})"
)
# 固化必须由 System 私有 queues 边界定义的符号。
SYSTEM_PRIVATE_SYMBOLS = {
    # 有界命令队列及请求/响应契约归此文件。
    APP_ROOT / "queues" / "agent_command_queue.rs": (
        # 队列本体必须继续定义在 System 私有边界。
        "pub(crate) struct AgentCommandQueue",
        # 请求契约必须继续定义在 System 私有边界。
        "pub(crate) enum AgentCommandRequest",
        # 响应契约必须继续定义在 System 私有边界。
        "pub(crate) enum AgentCommandResponse",
    ),
    # 每窗状态与执行端口归此文件。
    APP_ROOT / "queues" / "window_agent_state.rs": (
        # 每窗状态机必须继续定义在 System 私有边界。
        "pub(crate) struct WindowAgentState",
        # 执行能力端口必须继续定义在 System 私有边界。
        "pub(crate) trait AgentCommandExecutor",
    ),
}


# 收集需要审查的全部 Rust 源码。
def guarded_rust_sources() -> list[Path]:
    # 初始化稳定的源码路径列表。
    sources: list[Path] = []
    # 逐目录收集 Rust 源码。
    for directory in GUARDED_DIRECTORIES:
        # 保持仓库路径顺序，令失败诊断稳定。
        sources.extend(sorted(directory.rglob("*.rs")))
    # 加入 System 私有的单文件语义边界。
    sources.extend(GUARDED_FILES)
    # 返回去重后的稳定路径集合。
    return sorted(set(sources))


# 定义 app Module 隔离回归测试。
class AppModuleBoundaryTests(unittest.TestCase):
    # 确认消费者与契约边界不直接引用 agent Module 实现。
    def test_window_event_loop_and_contracts_do_not_reference_agent_module(self) -> None:
        # 收集所有违反兄弟 Module 隔离的源码路径。
        violations: list[str] = []
        # 逐文件检查 Rust 模块路径。
        for path in guarded_rust_sources():
            # 读取当前源码并移除排版空白。
            compact = "".join(path.read_text(encoding="utf-8").split())
            # 只记录实际出现的 agent Module 路径。
            if FORBIDDEN_AGENT_REFERENCE.search(compact):
                # 保存仓库相对路径，便于直接定位回归。
                violations.append(path.relative_to(ROOT).as_posix())
        # 兄弟 Module 与 System 私有契约都不得反向引用 agent 实现。
        self.assertEqual(violations, [])

    # 确认跨 Module 命令契约仍由 app System 私有边界定义。
    def test_agent_command_contracts_remain_in_system_private_boundary(self) -> None:
        # 逐文件验证其拥有的稳定符号集合。
        for path, symbols in SYSTEM_PRIVATE_SYMBOLS.items():
            # 读取权威实现文件。
            source = path.read_text(encoding="utf-8")
            # 逐符号确认定义没有被移回 agent Module。
            for symbol in symbols:
                # 子测试保留文件与符号上下文。
                with self.subTest(path=path.relative_to(ROOT).as_posix(), symbol=symbol):
                    # System 私有边界必须继续拥有该定义。
                    self.assertIn(symbol, source)


# 允许直接执行当前测试文件。
if __name__ == "__main__":
    # 使用标准 unittest 入口执行全部用例。
    unittest.main()
