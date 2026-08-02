# -*- coding: utf-8 -*-
"""Keep macOS text-input Objective-C callbacks behind the owner failure guard."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
TEXT_INPUT = ROOT / "src/native/backends/macos/text_input_view.rs"
PLATFORM = ROOT / "src/native/backends/macos/platform.rs"

CALLBACKS = (
    "view_dealloc",
    "accepts_first_responder",
    "has_marked_text",
    "marked_range",
    "selected_range",
    "set_marked_text",
    "insert_text",
    "unmark_text",
    "key_down",
)


def callback_body(source: str, name: str) -> str:
    marker = f'unsafe extern "C" fn {name}'
    start = source.index(marker)
    end = source.find('\n    unsafe extern "C" fn', start + len(marker))
    return source[start:] if end < 0 else source[start:end]


class MacosCallbackGuardTests(unittest.TestCase):
    def test_every_text_input_objc_callback_has_an_abi_panic_boundary(self) -> None:
        source = TEXT_INPUT.read_text(encoding="utf-8")
        self.assertIn("pending_failures: PendingFailureSource", source)
        self.assertIn("catch_unwind", source)
        for name in CALLBACKS:
            body = callback_body(source, name)
            with self.subTest(callback=name):
                if name == "view_dealloc":
                    self.assertIn("catch_unwind", body)
                else:
                    self.assertIn("with_callback", body)

    def test_platform_forwards_the_pending_source_to_content_views(self) -> None:
        platform = PLATFORM.read_text(encoding="utf-8")
        self.assertIn("pending_failures: PendingFailureSource", platform)
        self.assertIn("self.pending_failures.clone()", platform)
        self.assertIn("pending_failures,\n        )", platform)


if __name__ == "__main__":
    unittest.main()
