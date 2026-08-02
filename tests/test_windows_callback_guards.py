# -*- coding: utf-8 -*-
"""Keep synchronous Windows display callbacks behind an ABI panic boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
PLATFORM_WINDOWS = ROOT / "src/platform/imp/windows.rs"
LEGACY_DISPLAY = ROOT / "src/native/backends/windows/display.rs"


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


if __name__ == "__main__":
    unittest.main()
