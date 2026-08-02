# -*- coding: utf-8 -*-
"""Keep Linux timer worker failures on the platform owner boundary."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TIMER = ROOT / "src/native/backends/linux/timer.rs"
PLATFORM = ROOT / "src/native/backends/linux/platform.rs"


class LinuxTimerGuardTests(unittest.TestCase):
    def test_worker_failures_use_the_pending_source(self) -> None:
        source = TIMER.read_text(encoding="utf-8")
        self.assertIn("PendingFailureSource", source)
        self.assertIn("RecvTimeoutError::Disconnected", source)
        self.assertIn("failed to start timer worker", source)
        self.assertIn("event queue lock was poisoned", source)
        self.assertIn("pending_failures.enqueue", source)

    def test_linux_platform_shares_source_with_wayland_and_timer(self) -> None:
        source = PLATFORM.read_text(encoding="utf-8")
        self.assertIn("WaylandBackend::new(pending_source.clone())", source)
        self.assertIn("LinuxTimer::new(timer_eq, pending_source)", source)


if __name__ == "__main__":
    unittest.main()
