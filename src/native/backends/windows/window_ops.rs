// ============================================================================
// platform/windows/window_ops.rs — Windows 平台窗口操作
//
// WindowsWindowOps 实现 WindowOps trait，封装 Win32 API 窗口调用。
// 与 PlatformWindowCore<WindowsWindowOps> 组合使用。
// ============================================================================

#![cfg(windows)]

use crate::core::error::{Errc, Error, Result};
use crate::diagnostics::PendingFailureSource;
// Windows 不解释激活身份，但仍实现统一的平台窗口契约。
use crate::native::windowing::event::{FrameRequestToken, PointerActivationId};
use crate::native::windowing::shared::WindowOps;
use crate::native::windowing::window::NativeFrameRequest;

use super::frame_pacer::{SharedWindowsFramePacerState, WindowsFramePacer};
use super::platform::WindowBinding;
use super::window_icon::WindowIconState;

fn screen_point_lparam(point: super::bindings::POINT) -> isize {
    let x = point.x as i16 as u16;
    let y = point.y as i16 as u16;
    (u32::from(x) | (u32::from(y) << 16)) as isize
}

fn centered_axis_origin(area_start: i32, area_end: i32, window_extent: i32) -> i32 {
    let start = i64::from(area_start);
    let centered = start + (i64::from(area_end) - start - i64::from(window_extent)) / 2;
    centered.clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn monitor_info_for_window(
    hwnd: *mut std::ffi::c_void,
    operation: &str,
) -> Result<super::bindings::MONITORINFO> {
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        let context = format!("{operation}: MonitorFromWindow failed");
        return Err(super::util::windows_diag(Errc::PlatformError, &context));
    }
    let mut info = super::bindings::MONITORINFO {
        cbSize: std::mem::size_of::<super::bindings::MONITORINFO>() as u32,
        rcMonitor: super::bindings::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: super::bindings::RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        let context = format!("{operation}: GetMonitorInfoW failed");
        return Err(super::util::windows_diag(Errc::PlatformError, &context));
    }
    Ok(info)
}

#[derive(Clone, Copy)]
struct FullscreenRestore {
    style: i32,
    placement: super::bindings::WINDOWPLACEMENT,
}

pub(crate) fn get_window_long_checked(
    hwnd: *mut std::ffi::c_void,
    index: i32,
    operation: &str,
) -> Result<i32> {
    // Win32 以零同时表示合法值与失败，必须先清空 last-error 再判定。
    unsafe {
        SetLastError(0);
        let value = GetWindowLongW(hwnd, index);
        let windows_code = GetLastError();
        if value == 0 && windows_code != 0 {
            return Err(super::util::windows_diag_for_code(
                Errc::PlatformError,
                operation,
                windows_code,
            ));
        }
        Ok(value)
    }
}

pub(crate) fn set_window_long_checked(
    hwnd: *mut std::ffi::c_void,
    index: i32,
    value: i32,
    operation: &str,
) -> Result<()> {
    // SetWindowLongW 返回旧值；旧值为零时只能通过 last-error 区分成败。
    unsafe {
        SetLastError(0);
        let previous = SetWindowLongW(hwnd, index, value);
        let windows_code = GetLastError();
        if previous == 0 && windows_code != 0 {
            return Err(super::util::windows_diag_for_code(
                Errc::PlatformError,
                operation,
                windows_code,
            ));
        }
    }
    Ok(())
}

/// Windows 平台窗口操作句柄。
///
/// 持有原生窗口句柄 HWND，所有 `os_*` 方法通过 Win32 API 操作窗口。
/// 仅保留还原 Win32 原生属性所需的平台状态；逻辑状态由 PlatformWindowCore 管理。
pub(crate) struct WindowsWindowOps {
    hwnd: *mut std::ffi::c_void,
    icon_state: WindowIconState,
    _binding: Box<WindowBinding>,
    frame_pacer: WindowsFramePacer,
    opacity_layered_style_owned: bool,
    fullscreen_restore: Option<FullscreenRestore>,
}

impl WindowsWindowOps {
    pub(crate) fn new(
        hwnd: *mut std::ffi::c_void,
        binding: Box<WindowBinding>,
        frame_pacer_state: SharedWindowsFramePacerState,
        pending_failures: PendingFailureSource,
    ) -> Self {
        Self {
            hwnd,
            icon_state: WindowIconState::new(hwnd),
            _binding: binding,
            frame_pacer: WindowsFramePacer::new(hwnd, frame_pacer_state, pending_failures),
            opacity_layered_style_owned: false,
            fullscreen_restore: None,
        }
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

    fn validate_logical_track_size(&self, operation: &str, width: i32, height: i32) -> Result<()> {
        self.ensure_valid_window(operation)?;
        let style_operation = format!("{operation}: GetWindowLongW(GWL_STYLE) failed");
        let style = get_window_long_checked(self.hwnd, GWL_STYLE, &style_operation)? as u32;
        let ex_style_operation = format!("{operation}: GetWindowLongW(GWL_EXSTYLE) failed");
        let ex_style = get_window_long_checked(self.hwnd, GWL_EXSTYLE, &ex_style_operation)? as u32;
        super::dpi::outer_size_for_logical_client(
            width,
            height,
            style,
            ex_style,
            super::dpi::dpi_for_window(self.hwnd),
        )?;
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

    fn os_request_close(&mut self) -> Result<()> {
        self.ensure_valid_window("os_request_close")?;
        if unsafe { PostMessageW(self.hwnd, WM_CLOSE, 0, 0) } == 0 {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_request_close: PostMessageW failed",
            ));
        }
        Ok(())
    }

    // Windows 保持现有非客户区拖动消息路径。
    fn os_begin_move_drag(
        // Windows 使用当前 Win32 捕获状态，不消费 Wayland 式授权。
        &mut self,
        // 保留统一签名并明确忽略不可解释身份。
        _pointer_activation: Option<PointerActivationId>,
    // 返回原有 Win32 消息提交结果。
    ) -> Result<()> {
        self.ensure_valid_window("os_begin_move_drag")?;
        unsafe {
            ReleaseCapture();
            if PostMessageW(self.hwnd, WM_NCLBUTTONDOWN, HTCAPTION, 0) == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_begin_move_drag: PostMessageW failed",
                ));
            }
        }
        Ok(())
    }

    fn os_show_system_menu(&mut self) -> Result<()> {
        self.ensure_valid_window("os_show_system_menu")?;
        let mut cursor = super::bindings::POINT { x: 0, y: 0 };
        // SAFETY: `cursor` 在调用期间保持有效可写，且 HWND 已由 `ensure_valid_window` 验证。
        unsafe {
            if GetCursorPos(&mut cursor) == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_show_system_menu: GetCursorPos failed",
                ));
            }
            if PostMessageW(
                self.hwnd,
                WM_NCRBUTTONUP,
                HTCAPTION,
                screen_point_lparam(cursor),
            ) == 0
            {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_show_system_menu: PostMessageW failed",
                ));
            }
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

    fn os_set_icon(&mut self, path: &str) -> Result<()> {
        self.ensure_valid_window("os_set_icon")?;
        self.icon_state.set_from_file(path)
    }

    fn os_flash(&mut self) -> Result<()> {
        self.ensure_valid_window("os_flash")?;
        let info = super::bindings::FLASHWINFO {
            cbSize: std::mem::size_of::<super::bindings::FLASHWINFO>() as u32,
            hwnd: self.hwnd,
            dwFlags: FLASHW_TRAY | FLASHW_TIMERNOFG,
            uCount: 3,
            dwTimeout: 0,
        };
        // SAFETY: info is a fully initialized FLASHWINFO for a live HWND. The
        // BOOL result reports the previous active state, not call success.
        unsafe {
            FlashWindowEx(&info);
        }
        Ok(())
    }

    fn os_center_on_screen(&mut self) -> Result<()> {
        self.ensure_valid_window("os_center_on_screen")?;
        unsafe {
            let monitor = monitor_info_for_window(self.hwnd, "os_center_on_screen")?;
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
            let x = centered_axis_origin(monitor.rcWork.left, monitor.rcWork.right, w);
            let y = centered_axis_origin(monitor.rcWork.top, monitor.rcWork.bottom, h);
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
        let hwnd = self.hwnd;
        super::dpi::with_per_monitor_v2(|| {
            let style = get_window_long_checked(
                hwnd,
                GWL_STYLE,
                "os_set_size: GetWindowLongW(GWL_STYLE) failed",
            )? as u32;
            let ex_style = get_window_long_checked(
                hwnd,
                GWL_EXSTYLE,
                "os_set_size: GetWindowLongW(GWL_EXSTYLE) failed",
            )? as u32;
            let dpi = super::dpi::dpi_for_window(hwnd);
            let (outer_width, outer_height) =
                super::dpi::outer_size_for_logical_client(w, h, style, ex_style, dpi)?;
            if unsafe {
                SetWindowPos(
                    hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    outer_width,
                    outer_height,
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
        })
    }

    fn os_set_min_size(&mut self, width: i32, height: i32) -> Result<()> {
        self.validate_logical_track_size("os_set_min_size", width, height)
    }

    fn os_set_max_size(&mut self, width: i32, height: i32) -> Result<()> {
        self.validate_logical_track_size("os_set_max_size", width, height)
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
        let style = get_window_long_checked(
            self.hwnd,
            GWL_STYLE,
            "os_set_resizable: GetWindowLongW failed",
        )? as u32;
        let resize_style = WS_THICKFRAME | WS_MAXIMIZEBOX;
        let new_style = if resizable {
            style | resize_style
        } else {
            style & !resize_style
        };
        set_window_long_checked(
            self.hwnd,
            GWL_STYLE,
            new_style as i32,
            "os_set_resizable: SetWindowLongW failed",
        )?;
        unsafe {
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

    fn os_set_system_title_bar_visible(&mut self, visible: bool) -> Result<()> {
        self.ensure_valid_window("os_set_system_title_bar_visible")?;
        let style = get_window_long_checked(
            self.hwnd,
            GWL_STYLE,
            "os_set_system_title_bar_visible: GetWindowLongW failed",
        )? as u32;
        let new_style = if visible {
            style | WS_CAPTION
        } else {
            style & !WS_CAPTION
        };
        set_window_long_checked(
            self.hwnd,
            GWL_STYLE,
            new_style as i32,
            "os_set_system_title_bar_visible: SetWindowLongW failed",
        )?;
        unsafe {
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
                    "os_set_system_title_bar_visible: SetWindowPos failed",
                ));
            }
        }
        // 去掉系统标题栏后恢复 DWM 阴影/圆角；恢复标题栏时清掉扩展边距。
        let maximized = super::custom_chrome::is_effectively_maximized(
            self.hwnd,
            new_style,
            self._binding.state.borrow().maximized,
        );
        super::custom_chrome::apply_dwm_frame_effects(self.hwnd, new_style, maximized)
    }

    fn os_set_borderless(&mut self, borderless: bool) -> Result<()> {
        self.ensure_valid_window("os_set_borderless")?;
        let style = get_window_long_checked(
            self.hwnd,
            GWL_STYLE,
            "os_set_borderless: GetWindowLongW failed",
        )? as u32;
        let new_style = if borderless {
            style & !WS_OVERLAPPEDWINDOW
        } else {
            style | WS_OVERLAPPEDWINDOW
        };
        set_window_long_checked(
            self.hwnd,
            GWL_STYLE,
            new_style as i32,
            "os_set_borderless: SetWindowLongW failed",
        )?;
        unsafe {
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
            if self.fullscreen_restore.is_some() {
                return Ok(());
            }
            let style = get_window_long_checked(
                self.hwnd,
                GWL_STYLE,
                "os_set_fullscreen: GetWindowLongW failed",
            )?;
            let mut placement = super::bindings::WINDOWPLACEMENT {
                length: std::mem::size_of::<super::bindings::WINDOWPLACEMENT>() as u32,
                flags: 0,
                showCmd: 0,
                ptMinPosition: super::bindings::POINT { x: 0, y: 0 },
                ptMaxPosition: super::bindings::POINT { x: 0, y: 0 },
                rcNormalPosition: super::bindings::RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
            };
            if unsafe { GetWindowPlacement(self.hwnd, &mut placement) } == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_fullscreen: GetWindowPlacement failed",
                ));
            }
            let monitor = monitor_info_for_window(self.hwnd, "os_set_fullscreen")?;
            let fullscreen_style = (style as u32 & !WS_OVERLAPPEDWINDOW) | WS_POPUP;
            set_window_long_checked(
                self.hwnd,
                GWL_STYLE,
                fullscreen_style as i32,
                "os_set_fullscreen: SetWindowLongW failed",
            )?;
            let fullscreen_positioned = unsafe {
                SetWindowPos(
                    self.hwnd,
                    std::ptr::null_mut(),
                    monitor.rcMonitor.left,
                    monitor.rcMonitor.top,
                    monitor.rcMonitor.right - monitor.rcMonitor.left,
                    monitor.rcMonitor.bottom - monitor.rcMonitor.top,
                    SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                )
            };
            if fullscreen_positioned == 0 {
                let error = super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_fullscreen: SetWindowPos failed",
                );
                let _ = set_window_long_checked(
                    self.hwnd,
                    GWL_STYLE,
                    style,
                    "os_set_fullscreen: rollback SetWindowLongW failed",
                );
                let _ = unsafe { SetWindowPlacement(self.hwnd, &placement) };
                if style as u32 & WS_VISIBLE == 0 {
                    unsafe {
                        ShowWindow(self.hwnd, SW_HIDE);
                    }
                }
                return Err(error);
            }
            self.fullscreen_restore = Some(FullscreenRestore { style, placement });
        } else {
            let Some(restore) = self.fullscreen_restore else {
                return Ok(());
            };
            let current_style = get_window_long_checked(
                self.hwnd,
                GWL_STYLE,
                "os_set_fullscreen: restore GetWindowLongW failed",
            )? as u32;
            let was_visible = current_style & WS_VISIBLE != 0;
            let restored_style =
                (restore.style as u32 & !WS_VISIBLE) | (current_style & WS_VISIBLE);
            set_window_long_checked(
                self.hwnd,
                GWL_STYLE,
                restored_style as i32,
                "os_set_fullscreen: SetWindowLongW failed",
            )?;
            if unsafe { SetWindowPlacement(self.hwnd, &restore.placement) } == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_fullscreen: SetWindowPlacement failed",
                ));
            }
            if !was_visible {
                unsafe {
                    ShowWindow(self.hwnd, SW_HIDE);
                }
            }
            if unsafe {
                SetWindowPos(
                    self.hwnd,
                    std::ptr::null_mut(),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_NOOWNERZORDER | SWP_FRAMECHANGED,
                )
            } == 0
            {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_fullscreen: restore SetWindowPos failed",
                ));
            }
            self.fullscreen_restore = None;
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
            let ex_style = get_window_long_checked(
                self.hwnd,
                GWL_EXSTYLE,
                "os_set_opacity: GetWindowLongW failed",
            )? as u32;
            let adds_layered_style = ex_style & WS_EX_LAYERED == 0;
            if adds_layered_style {
                set_window_long_checked(
                    self.hwnd,
                    GWL_EXSTYLE,
                    (ex_style | WS_EX_LAYERED) as i32,
                    "os_set_opacity: SetWindowLongW failed",
                )?;
            }
            unsafe {
                if SetLayeredWindowAttributes(
                    self.hwnd,
                    0,
                    (opacity * 255.0).round() as u8,
                    LWA_ALPHA,
                ) == 0
                {
                    if adds_layered_style {
                        let _ = set_window_long_checked(
                            self.hwnd,
                            GWL_EXSTYLE,
                            ex_style as i32,
                            "os_set_opacity: rollback SetWindowLongW failed",
                        );
                    }
                    return Err(super::util::windows_diag(
                        Errc::PlatformError,
                        "os_set_opacity: SetLayeredWindowAttributes failed",
                    ));
                }
            }
            self.opacity_layered_style_owned |= adds_layered_style;
        } else if self.opacity_layered_style_owned {
            if unsafe { SetLayeredWindowAttributes(self.hwnd, 0, u8::MAX, LWA_ALPHA) } == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_opacity: restore SetLayeredWindowAttributes failed",
                ));
            }
            let ex_style = get_window_long_checked(
                self.hwnd,
                GWL_EXSTYLE,
                "os_set_opacity: restore GetWindowLongW failed",
            )? as u32;
            set_window_long_checked(
                self.hwnd,
                GWL_EXSTYLE,
                (ex_style & !WS_EX_LAYERED) as i32,
                "os_set_opacity: restore SetWindowLongW failed",
            )?;
            self.opacity_layered_style_owned = false;
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

    fn os_request_native_frame(&mut self, request: NativeFrameRequest) -> Result<bool> {
        self.ensure_valid_window("os_request_native_frame")?;
        self.frame_pacer.request(request)
    }

    fn os_native_frame_presented(&mut self, token: FrameRequestToken) -> Result<()> {
        self.ensure_valid_window("os_native_frame_presented")?;
        self.frame_pacer.presented(token)
    }

    fn os_cancel_native_frame(&mut self, token: FrameRequestToken) -> Result<()> {
        self.frame_pacer.cancel(token);
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
