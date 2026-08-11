# -*- coding: utf-8 -*-
"""Keep Wayland dispatch panics inside the owner-thread failure boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVENT_LOOP = ROOT / "src/native/backends/linux/wayland/event_loop.rs"
COMPAT = ROOT / "src/native/backends/linux/wayland/compat.rs"
# 读取 Linux 平台的 owner-thread 失败提取边界。
PLATFORM = ROOT / "src/native/backends/linux/platform.rs"
# 定位 Wayland 窗口操作与装饰模式实现。
WINDOW_OPS = ROOT / "src/native/backends/linux/wayland/window_ops.rs"
# 定位实际启用自定义标题栏的 GUI 演示入口。
GUI_DEMO = ROOT / "demo/gui-demo/src/gui/mod.rs"


class WaylandCallbackGuardTests(unittest.TestCase):
    def test_dispatch_pending_catches_callback_panics(self) -> None:
        source = EVENT_LOOP.read_text(encoding="utf-8")
        self.assertIn("catch_unwind", source)
        self.assertIn("event_queue.dispatch_pending", source)
        self.assertIn("callback panicked during dispatch", source)
        self.assertIn("Errc::PlatformError", source)

    def test_unknown_created_child_remains_an_explicit_failure_case(self) -> None:
        source = COMPAT.read_text(encoding="utf-8")
        self.assertIn("fn event_created_child", source)
        self.assertIn("missing event-created child dispatch", source)

    # 确认 Wayland dispatch 与关闭失败最终都进入 owner-thread 队列。
    def test_dispatch_failures_reach_owner_pending_source(self) -> None:
        # 读取兼容层的 callback source 持有与入队路径。
        compat = COMPAT.read_text(encoding="utf-8")
        # 读取事件循环的 panic/IO 失败收口路径。
        event_loop = EVENT_LOOP.read_text(encoding="utf-8")
        # 读取 Linux 平台向应用暴露的失败提取路径。
        platform = PLATFORM.read_text(encoding="utf-8")
        # 兼容层必须持有独立的 pending source。
        self.assertIn("pending_failures: PendingFailureSource", compat)
        # callback panic 必须把 typed failure 放入该 source。
        self.assertIn("self.pending_failures.enqueue", compat)
        # 事件循环必须用 checked dispatch 捕获异常并走关闭收口。
        self.assertIn("fn dispatch_pending_checked", event_loop)
        # 所有关闭失败必须通过 close_after_failure 入队。
        self.assertIn("close_after_failure", event_loop)
        # owner-thread 必须从同一 backend 提取 pending failure。
        self.assertIn("fn take_pending_failure", platform)
        # Linux 平台不得绕过 Wayland backend 的 source。
        self.assertIn("self.backend.take_pending_failure()", platform)

    # 确认 Wayland 自定义标题栏由客户端装饰模式承接且演示不再按平台禁用。
    def test_custom_title_bar_uses_wayland_client_side_decoration(self) -> None:
        # 读取 Wayland 窗口操作实现。
        window_ops = WINDOW_OPS.read_text(encoding="utf-8")
        # 读取 GUI 演示的窗口配置。
        gui_demo = GUI_DEMO.read_text(encoding="utf-8")
        # Wayland 后端必须实现统一的系统标题栏可见性能力。
        self.assertIn("fn os_set_system_title_bar_visible", window_ops)
        # 隐藏系统标题栏必须请求客户端装饰。
        self.assertIn("XdgDecoMode::ClientSide", window_ops)
        # 恢复系统标题栏必须请求服务端装饰。
        self.assertIn("XdgDecoMode::ServerSide", window_ops)
        # 关闭窗口时必须先释放依赖顶层窗口的装饰对象。
        close_start = window_ops.index("fn os_close")
        # 以窗口外观分区标记限定关闭实现片段。
        close_end = window_ops.index("// ── 窗口外观", close_start)
        # 保存关闭实现，避免其他生命周期赋值影响断言。
        close_source = window_ops[close_start:close_end]
        # 装饰对象必须在顶层窗口之前释放。
        self.assertLess(
            # 获取装饰释放语句位置。
            close_source.index("self.xdg_decoration = None"),
            # 获取顶层窗口释放语句位置。
            close_source.index("self.toplevel = None"),
        )
        # 演示必须实际启用自定义标题栏。
        self.assertIn(".custom_title_bar(true);", gui_demo)
        # 不得恢复仅 Windows 启用的旧分支。
        self.assertNotIn("#[cfg(windows)]\n    let app = App::new()", gui_demo)
        # 不得恢复非 Windows 跳过自定义标题栏的旧分支。
        self.assertNotIn("#[cfg(not(windows))]\n    let app = App::new().title", gui_demo)


if __name__ == "__main__":
    unittest.main()
