// ============================================================================
// platform/windows/window_ops.rs — Windows 平台窗口操作
//
// WindowsWindowOps 实现 WindowOps trait，封装 Win32 API 窗口调用。
// 与 PlatformWindowCore<WindowsWindowOps> 组合使用。
// ============================================================================

#![cfg(windows)]

use crate::core::WindowOps;

/// Windows 平台窗口操作句柄。
///
/// 持有原生窗口句柄 HWND，所有 `os_*` 方法通过 Win32 API 操作窗口。
/// 不持有窗口状态（状态通过 PlatformWindowCore 的共享引用管理）。
pub(crate) struct WindowsWindowOps {
    hwnd: *mut std::ffi::c_void,
}

impl WindowsWindowOps {
    pub(crate) fn new(hwnd: *mut std::ffi::c_void) -> Self {
        Self { hwnd }
    }

    /// 获取 HWND（供内部使用）。
    pub(crate) fn hwnd(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps 实现
// ════════════════════════════════════════════════════════════════════════════

use super::ffi::*;
use super::consts::*;

impl WindowOps for WindowsWindowOps {
    // ── 窗口生命周期 ──────────────────────────────────────

    fn os_show(&mut self) {
        unsafe { ShowWindow(self.hwnd, SW_SHOWNORMAL); }
    }

    fn os_hide(&mut self) {
        unsafe { ShowWindow(self.hwnd, SW_HIDE); }
    }

    fn os_close(&mut self) {
        unsafe { DestroyWindow(self.hwnd); }
    }

    // ── 窗口外观 ──────────────────────────────────────────

    fn os_set_title(&mut self, title: &str) {
        let wide = super::util::to_wide(title);
        unsafe { SetWindowTextW(self.hwnd, wide.as_ptr()); }
    }

    fn os_center_on_screen(&mut self) {
        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let mut rect = super::bindings::RECT { left: 0, top: 0, right: 0, bottom: 0 };
            if GetWindowRect(self.hwnd, &mut rect) != 0 {
                let w = rect.right - rect.left;
                let h = rect.bottom - rect.top;
                let x = (sw - w) / 2;
                let y = (sh - h) / 2;
                SetWindowPos(
                    self.hwnd,
                    std::ptr::null_mut(),
                    x, y, 0, 0,
                    SWP_NOSIZE | SWP_NOZORDER,
                );
            }
        }
    }

    fn os_raise(&mut self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOP as *mut std::ffi::c_void,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
            );
        }
    }

    fn os_lower(&mut self) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_BOTTOM as *mut std::ffi::c_void,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    // ── 尺寸/位置 ─────────────────────────────────────────

    fn os_set_size(&mut self, w: i32, h: i32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0, 0, w, h,
                SWP_NOMOVE | SWP_NOZORDER,
            );
        }
    }

    fn os_set_min_size(&mut self, _w: i32, _h: i32) {
        // Windows: 通过 WM_GETMINMAXINFO 处理，在 wnd_proc 中实现
    }

    fn os_set_max_size(&mut self, _w: i32, _h: i32) {
        // Windows: 通过 WM_GETMINMAXINFO 处理，在 wnd_proc 中实现
    }

    fn os_set_position(&mut self, x: i32, y: i32) {
        unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x, y, 0, 0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    // ── 窗口状态 ──────────────────────────────────────────

    fn os_set_resizable(&mut self, resizable: bool) {
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if resizable {
                style | WS_THICKFRAME
            } else {
                style & !WS_THICKFRAME
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn os_maximize(&mut self) {
        unsafe { ShowWindow(self.hwnd, SW_MAXIMIZE); }
    }

    fn os_minimize(&mut self) {
        unsafe { ShowWindow(self.hwnd, SW_MINIMIZE); }
    }

    fn os_restore(&mut self) {
        unsafe { ShowWindow(self.hwnd, SW_RESTORE); }
    }

    fn os_set_borderless(&mut self, borderless: bool) {
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if borderless {
                style & !WS_OVERLAPPEDWINDOW
            } else {
                style | WS_OVERLAPPEDWINDOW
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    fn os_set_fullscreen(&mut self, fullscreen: bool) {
        if fullscreen {
            unsafe {
                SetWindowLongW(self.hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let sh = GetSystemMetrics(SM_CYSCREEN);
                SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST as *mut std::ffi::c_void,
                    0, 0, sw, sh,
                    SWP_FRAMECHANGED,
                );
            }
        } else {
            unsafe {
                let flags = WS_OVERLAPPEDWINDOW | WS_VISIBLE;
                // Note: resizable state is managed by the caller before this call
                SetWindowLongW(self.hwnd, GWL_STYLE, flags as i32);
                SetWindowPos(
                    self.hwnd,
                    HWND_NOTOPMOST as *mut std::ffi::c_void,
                    0, 0, 0, 0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
                );
            }
        }
    }

    fn os_set_always_on_top(&mut self, on: bool) {
        unsafe {
            let pos = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(
                self.hwnd,
                pos as *mut std::ffi::c_void,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
        }
    }

    fn os_set_opacity(&mut self, opacity: f32) {
        if opacity < 1.0 {
            unsafe {
                let ex_style = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, (ex_style | WS_EX_LAYERED) as i32);
                SetLayeredWindowAttributes(self.hwnd, 0, (opacity * 255.0) as u8, LWA_ALPHA);
            }
        }
    }

    // ── 特性开关 ──────────────────────────────────────────

    fn os_start_text_input(&mut self) {
        // Windows IME: managed via WM_IME_* messages
    }

    fn os_stop_text_input(&mut self) {
        // Windows IME: managed via WM_IME_* messages
    }

    fn os_enable_file_drop(&mut self, enable: bool) {
        unsafe {
            DragAcceptFiles(self.hwnd, if enable { TRUE } else { FALSE });
        }
    }

    // ── 原生句柄 ──────────────────────────────────────────

    fn native_handle(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

use super::consts::TRUE;
use super::consts::FALSE;
