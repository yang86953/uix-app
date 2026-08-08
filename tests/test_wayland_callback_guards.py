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


if __name__ == "__main__":
    unittest.main()
