# -*- coding: utf-8 -*-
"""Keep Wayland dispatch panics inside the owner-thread failure boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
EVENT_LOOP = ROOT / "src/native/backends/linux/wayland/event_loop.rs"
COMPAT = ROOT / "src/native/backends/linux/wayland/compat.rs"


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


if __name__ == "__main__":
    unittest.main()
