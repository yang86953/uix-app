// ============================================================================
// platform/windows/window_ops.rs — Windows 平台窗口操作
//
// WindowsWindowOps 实现 WindowOps trait，封装 Win32 API 窗口调用。
// 与 PlatformWindowCore<WindowsWindowOps> 组合使用。
// ============================================================================

#![cfg(windows)]

use crate::core::error::{Errc, Error, Result};
use crate::native::shared::WindowOps;

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

    fn ensure_valid_window(&self, operation: &str) -> Result<()> {
        if self.hwnd.is_null() || unsafe { IsWindow(self.hwnd) } == 0 {
            return Err(Error::new(
                Errc::InvalidState,
                format!("{operation}: invalid Win32 window handle"),
            ));
        }
        Ok(())
    }
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps 实现
// ════════════════════════════════════════════════════════════════════════════

use super::consts::*;
use super::ffi::*;

impl WindowOps for WindowsWindowOps {
    // ── 窗口生命周期 ──────────────────────────────────────

    fn os_show(&mut self) -> Result<()> {
        self.ensure_valid_window("os_show")?;
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNORMAL);
        }
        Ok(())
    }

    fn os_hide(&mut self) -> Result<()> {
        self.ensure_valid_window("os_hide")?;
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
        Ok(())
    }

    fn os_close(&mut self) -> Result<()> {
        self.ensure_valid_window("os_close")?;
        if unsafe { DestroyWindow(self.hwnd) } == 0 {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_close: DestroyWindow failed",
            ));
        }
        Ok(())
    }

    // ── 窗口外观 ──────────────────────────────────────────

    fn os_set_title(&mut self, title: &str) -> Result<()> {
        self.ensure_valid_window("os_set_title")?;
        let wide = super::util::to_wide(title);
        if unsafe { SetWindowTextW(self.hwnd, wide.as_ptr()) } == 0 {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_set_title: SetWindowTextW failed",
            ));
        }
        Ok(())
    }

    fn os_center_on_screen(&mut self) -> Result<()> {
        self.ensure_valid_window("os_center_on_screen")?;
        unsafe {
            let sw = GetSystemMetrics(SM_CXSCREEN);
            let sh = GetSystemMetrics(SM_CYSCREEN);
            let mut rect = super::bindings::RECT {
                left: 0,
                top: 0,
                right: 0,
                bottom: 0,
            };
            if GetWindowRect(self.hwnd, &mut rect) == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_center_on_screen: GetWindowRect failed",
                ));
            }
            let w = rect.right - rect.left;
            let h = rect.bottom - rect.top;
            let x = (sw - w) / 2;
            let y = (sh - h) / 2;
            if SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER,
            ) == 0
            {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_center_on_screen: SetWindowPos failed",
                ));
            }
        }
        Ok(())
    }

    fn os_raise(&mut self) -> Result<()> {
        self.ensure_valid_window("os_raise")?;
        if unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_TOP as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
            )
        } == 0
        {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_raise: SetWindowPos failed",
            ));
        }
        Ok(())
    }

    fn os_lower(&mut self) -> Result<()> {
        self.ensure_valid_window("os_lower")?;
        if unsafe {
            SetWindowPos(
                self.hwnd,
                HWND_BOTTOM as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            )
        } == 0
        {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_lower: SetWindowPos failed",
            ));
        }
        Ok(())
    }

    // ── 尺寸/位置 ─────────────────────────────────────────

    fn os_set_size(&mut self, w: i32, h: i32) -> Result<()> {
        self.ensure_valid_window("os_set_size")?;
        if unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                w,
                h,
                SWP_NOMOVE | SWP_NOZORDER,
            )
        } == 0
        {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_set_size: SetWindowPos failed",
            ));
        }
        Ok(())
    }

    fn os_set_min_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        crate::native::shared::unimpl("os_set_min_size")
    }

    fn os_set_max_size(&mut self, _w: i32, _h: i32) -> Result<()> {
        crate::native::shared::unimpl("os_set_max_size")
    }

    fn os_set_position(&mut self, x: i32, y: i32) -> Result<()> {
        self.ensure_valid_window("os_set_position")?;
        if unsafe {
            SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            )
        } == 0
        {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_set_position: SetWindowPos failed",
            ));
        }
        Ok(())
    }

    // ── 窗口状态 ──────────────────────────────────────────

    fn os_set_resizable(&mut self, resizable: bool) -> Result<()> {
        self.ensure_valid_window("os_set_resizable")?;
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if resizable {
                style | WS_THICKFRAME
            } else {
                style & !WS_THICKFRAME
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
            if SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            ) == 0
            {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_resizable: SetWindowPos failed",
                ));
            }
        }
        Ok(())
    }

    fn os_maximize(&mut self) -> Result<()> {
        self.ensure_valid_window("os_maximize")?;
        unsafe {
            ShowWindow(self.hwnd, SW_MAXIMIZE);
        }
        Ok(())
    }

    fn os_minimize(&mut self) -> Result<()> {
        self.ensure_valid_window("os_minimize")?;
        unsafe {
            ShowWindow(self.hwnd, SW_MINIMIZE);
        }
        Ok(())
    }

    fn os_restore(&mut self) -> Result<()> {
        self.ensure_valid_window("os_restore")?;
        unsafe {
            ShowWindow(self.hwnd, SW_RESTORE);
        }
        Ok(())
    }

    fn os_set_borderless(&mut self, borderless: bool) -> Result<()> {
        self.ensure_valid_window("os_set_borderless")?;
        unsafe {
            let style = GetWindowLongW(self.hwnd, GWL_STYLE) as u32;
            let new_style = if borderless {
                style & !WS_OVERLAPPEDWINDOW
            } else {
                style | WS_OVERLAPPEDWINDOW
            };
            SetWindowLongW(self.hwnd, GWL_STYLE, new_style as i32);
            if SetWindowPos(
                self.hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            ) == 0
            {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_borderless: SetWindowPos failed",
                ));
            }
        }
        Ok(())
    }

    fn os_set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        self.ensure_valid_window("os_set_fullscreen")?;
        if fullscreen {
            unsafe {
                SetWindowLongW(self.hwnd, GWL_STYLE, (WS_POPUP | WS_VISIBLE) as i32);
                let sw = GetSystemMetrics(SM_CXSCREEN);
                let sh = GetSystemMetrics(SM_CYSCREEN);
                if SetWindowPos(
                    self.hwnd,
                    HWND_TOPMOST as *mut std::ffi::c_void,
                    0,
                    0,
                    sw,
                    sh,
                    SWP_FRAMECHANGED,
                ) == 0
                {
                    return Err(super::util::windows_diag(
                        Errc::PlatformError,
                        "os_set_fullscreen: SetWindowPos failed",
                    ));
                }
            }
        } else {
            unsafe {
                let flags = WS_OVERLAPPEDWINDOW | WS_VISIBLE;
                // Note: resizable state is managed by the caller before this call
                SetWindowLongW(self.hwnd, GWL_STYLE, flags as i32);
                if SetWindowPos(
                    self.hwnd,
                    HWND_NOTOPMOST as *mut std::ffi::c_void,
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_FRAMECHANGED,
                ) == 0
                {
                    return Err(super::util::windows_diag(
                        Errc::PlatformError,
                        "os_set_fullscreen: SetWindowPos failed",
                    ));
                }
            }
        }
        Ok(())
    }

    fn os_set_always_on_top(&mut self, on: bool) -> Result<()> {
        self.ensure_valid_window("os_set_always_on_top")?;
        if unsafe {
            let pos = if on { HWND_TOPMOST } else { HWND_NOTOPMOST };
            SetWindowPos(
                self.hwnd,
                pos as *mut std::ffi::c_void,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE,
            )
        } == 0
        {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_set_always_on_top: SetWindowPos failed",
            ));
        }
        Ok(())
    }

    fn os_set_opacity(&mut self, opacity: f32) -> Result<()> {
        self.ensure_valid_window("os_set_opacity")?;
        if opacity < 1.0 {
            unsafe {
                let ex_style = GetWindowLongW(self.hwnd, GWL_EXSTYLE) as u32;
                SetWindowLongW(self.hwnd, GWL_EXSTYLE, (ex_style | WS_EX_LAYERED) as i32);
                if SetLayeredWindowAttributes(self.hwnd, 0, (opacity * 255.0) as u8, LWA_ALPHA) == 0
                {
                    return Err(super::util::windows_diag(
                        Errc::PlatformError,
                        "os_set_opacity: SetLayeredWindowAttributes failed",
                    ));
                }
            }
        }
        Ok(())
    }

    // ── 特性开关 ──────────────────────────────────────────

    fn os_start_text_input(&mut self) -> Result<()> {
        self.ensure_valid_window("os_start_text_input")?;
        // Windows IME: managed via WM_IME_* messages
        Ok(())
    }

    fn os_stop_text_input(&mut self) -> Result<()> {
        self.ensure_valid_window("os_stop_text_input")?;
        // Windows IME: managed via WM_IME_* messages
        Ok(())
    }

    fn os_enable_file_drop(&mut self, enable: bool) -> Result<()> {
        self.ensure_valid_window("os_enable_file_drop")?;
        unsafe {
            DragAcceptFiles(self.hwnd, if enable { TRUE } else { FALSE });
        }
        Ok(())
    }

    // ── 原生句柄 ──────────────────────────────────────────

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }

    fn native_handle(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

use super::consts::FALSE;
use super::consts::TRUE;
