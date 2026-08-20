# -*- coding: utf-8 -*-
"""Keep synchronous Windows display callbacks behind an ABI panic boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
# 平台公开门面的 Windows Provider 归属 host 功能域。
PLATFORM_WINDOWS = ROOT / "src/platform/host/providers/windows.rs"
LEGACY_DISPLAY = ROOT / "src/native/backends/windows/display.rs"
# 读取 Windows 窗口过程的 owner-thread failure queue 边界。
WND_PROC = ROOT / "src/native/backends/windows/wnd_proc.rs"


class WindowsCallbackGuardTests(unittest.TestCase):
    def test_public_platform_display_callback_catches_panic(self) -> None:
        source = PLATFORM_WINDOWS.read_text(encoding="utf-8")
        wrapper_start = source.index('unsafe extern "system" fn collect_monitor')
        inner_start = source.index("unsafe fn collect_monitor_unchecked", wrapper_start)
        wrapper = source[wrapper_start:inner_start]

        self.assertIn("catch_unwind", wrapper)
        self.assertIn("AssertUnwindSafe", wrapper)
        self.assertIn("collect_monitor_unchecked", wrapper)
        self.assertIn("panicked = true", wrapper)
        self.assertIn("EnumDisplayMonitors callback panicked", source)
        self.assertNotIn("values.push", wrapper)

    def test_legacy_native_display_callback_catches_panic(self) -> None:
        source = LEGACY_DISPLAY.read_text(encoding="utf-8")
        wrapper_start = source.index('unsafe extern "system" fn collect_monitor')
        inner_start = source.index("unsafe fn collect_monitor_unchecked", wrapper_start)
        wrapper = source[wrapper_start:inner_start]

        self.assertIn("catch_unwind", wrapper)
        self.assertIn("AssertUnwindSafe", wrapper)
        self.assertIn("collect_monitor_unchecked", wrapper)
        self.assertIn("failed = true", wrapper)
        self.assertNotIn("monitors.push", wrapper)

    # 确认 wnd_proc ABI panic 在安全 fallback 外仍通知 owner failure queue。
    def test_wnd_proc_panic_reaches_owner_pending_source(self) -> None:
        # 读取窗口过程完整的 panic boundary 与通知实现。
        source = WND_PROC.read_text(encoding="utf-8")
        # 外层 ABI 边界必须调用可恢复的 run helper。
        self.assertIn("run_wnd_proc_boundary", source)
        # panic 分支必须调用显式的队列通知函数。
        self.assertIn("enqueue_wnd_proc_panic", source)
        # 通知函数必须通过 Windows platform owner 入队。
        self.assertIn("enqueue_callback_failure", source)


if __name__ == "__main__":
    unittest.main()
