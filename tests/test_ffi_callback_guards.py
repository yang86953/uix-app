# -*- coding: utf-8 -*-
"""Keep every remaining native callback boundary explicit and guarded."""
from __future__ import annotations

import unittest
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
WINDOWS_TIMER = ROOT / "src/native/backends/windows/timer.rs"
WINDOWS_DIALOG = ROOT / "src/native/backends/windows/file_dialog.rs"
WINDOWS_HELPERS = ROOT / "src/native/backends/windows/helpers.rs"
WINDOWS_WND_PROC = ROOT / "src/native/backends/windows/wnd_proc.rs"
MACOS_DELEGATE = ROOT / "src/native/backends/macos/window_delegate.rs"
MACOS_DISPLAY_LINK = ROOT / "src/native/backends/macos/display_link.rs"


class FfiCallbackGuardTests(unittest.TestCase):
    def test_optional_win32_callback_slots_are_explicitly_unregistered(self) -> None:
        timer = WINDOWS_TIMER.read_text(encoding="utf-8")
        dialog = WINDOWS_DIALOG.read_text(encoding="utf-8")
        helpers = WINDOWS_HELPERS.read_text(encoding="utf-8")
        wnd_proc = WINDOWS_WND_PROC.read_text(encoding="utf-8")

        self.assertIn("SetTimer(self.hwnd, id, interval_ms, None)", timer)
        self.assertIn("WM_TIMER", wnd_proc)
        self.assertIn("lpfnWndProc: Some(wnd_proc)", helpers)
        self.assertEqual(dialog.count("lpfnHook: ptr::null_mut()"), 2)
        self.assertIn("lpfn: None", dialog)

    def test_macos_delegate_callbacks_use_the_shared_panic_boundary(self) -> None:
        source = MACOS_DELEGATE.read_text(encoding="utf-8")

        for callback in (
            "window_will_close",
            "window_did_resize",
            "window_did_become_key",
            "window_did_resign_key",
            "window_did_change_occlusion_state",
        ):
            start = source.index(f'unsafe extern "C" fn {callback}')
            end = source.find('unsafe extern "C" fn ', start + 1)
            body = source[start:] if end == -1 else source[start:end]
            self.assertTrue(
                "with_context" in body
                or "push_focus_event" in body
                or "push_window_event" in body,
                callback,
            )

        self.assertIn("catch_unwind(AssertUnwindSafe(|| callback(&*context))", source)
        self.assertIn("pending_failures.enqueue", source)

    def test_macos_display_link_callbacks_use_the_shared_panic_boundary(self) -> None:
        source = MACOS_DISPLAY_LINK.read_text(encoding="utf-8")

        for callback in ("display_link_fired", "target_dealloc"):
            start = source.index(f'unsafe extern "C" fn {callback}')
            end = source.find('unsafe extern "C" fn ', start + 1)
            body = source[start:] if end == -1 else source[start:end]
            self.assertIn("catch_unwind", body, callback)
            self.assertIn("AssertUnwindSafe", body, callback)

        self.assertIn("pending_failures.enqueue", source)


if __name__ == "__main__":
    unittest.main()
