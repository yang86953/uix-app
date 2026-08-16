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
# 读取 macOS delegate 使用的共享窗口生命周期投递 helper。
WINDOW_LIFECYCLE = ROOT / "src/native/windowing/shared/window_lifecycle.rs"


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

    # 校验 macOS window callback 的同步投递失败进入 owner-thread failure queue。
    def test_macos_delegate_propagates_event_queue_delivery_failures(self) -> None:
        # 读取 delegate callback adapter。
        delegate = MACOS_DELEGATE.read_text(encoding="utf-8")
        # 读取 close/resize 的共享投递实现。
        lifecycle = WINDOW_LIFECYCLE.read_text(encoding="utf-8")
        # shared helper 必须暴露 typed Result，而不是静默吞掉 mutex poison。
        self.assertGreaterEqual(lifecycle.count(") -> Result<()>"), 2)
        # close 队列锁失败必须有稳定 typed 诊断。
        self.assertIn("native window close event queue mutex poisoned", lifecycle)
        # resize 队列锁失败必须有稳定 typed 诊断。
        self.assertIn("native window resize event queue mutex poisoned", lifecycle)
        # resize 状态借用冲突必须使用非 panic 的检查式路径。
        self.assertIn("state.try_borrow_mut().map_err", lifecycle)
        # 状态借用冲突必须有稳定 typed 诊断。
        self.assertIn("native window resize state is already borrowed", lifecycle)
        # 旧的 lock().map 丢弃模式不得恢复。
        self.assertNotIn("let _ = events", lifecycle)
        # callback 闭包必须返回 Result，由 adapter 统一转换为 failure queue 项。
        self.assertIn("FnOnce(&WindowDelegateContext) -> crate::core::Result<()>", delegate)
        # typed delivery failure 必须与 panic 分支分开处理。
        self.assertIn("Ok(Err(error))", delegate)
        # 原始 typed failure 必须进入既有 PendingFailureSource。
        self.assertIn("pending_failures.enqueue(error)", delegate)
        # 普通 focus/occlusion 投递也不得继续静默丢弃 lock 失败。
        self.assertNotIn(".map(|mut events| events.push_back(event))", delegate)

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
