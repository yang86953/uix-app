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
use crate::platform::windowing::event::{FrameRequestToken, PointerActivationId};
// Windows adapter 直接实现共享窗口核心所定义的私有操作契约。
use crate::native::windowing::shared::window::WindowOps;
use crate::platform::windowing::window::{NativeFrameRequest, WindowOcclusionState};
use crate::platform::windowing::{WindowCapabilities, WindowCapability, WindowResizeEdge};

use super::frame_pacer::{SharedWindowsFramePacerState, WindowsFramePacer};
use super::platform::WindowBinding;
use super::window_icon::WindowIconState;

const WINDOWS_WINDOW_CAPABILITIES: WindowCapabilities = WindowCapabilities::from_slice(&[
    WindowCapability::RequestClose,
    WindowCapability::BeginMoveDrag,
    WindowCapability::BeginResizeDrag,
    WindowCapability::ShowSystemMenu,
    WindowCapability::CenterOnScreen,
    WindowCapability::Raise,
    WindowCapability::Lower,
    WindowCapability::SetWindowIcon,
    WindowCapability::FlashWindow,
    WindowCapability::ResizeNotify,
    WindowCapability::SetMinimumSize,
    WindowCapability::SetMaximumSize,
    WindowCapability::SetPosition,
    WindowCapability::SetResizable,
    WindowCapability::Maximize,
    WindowCapability::Minimize,
    WindowCapability::Restore,
    WindowCapability::ShowSystemTitleBar,
    WindowCapability::HideSystemTitleBar,
    WindowCapability::SetBorderless,
    WindowCapability::SetFullscreen,
    WindowCapability::SetAlwaysOnTop,
    WindowCapability::SetWindowOpacity,
    WindowCapability::StartTextInput,
    WindowCapability::StopTextInput,
    WindowCapability::EnableFileDrop,
    WindowCapability::DisableFileDrop,
    WindowCapability::RequestNativeFrame,
    WindowCapability::NativeFramePresented,
    WindowCapability::CancelNativeFrame,
    WindowCapability::ExactClientLogicalExtent,
    WindowCapability::NativeSurface,
]);

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
    // SAFETY: hwnd 为调用方传入的窗口句柄，MonitorFromWindow 只读取它；MONITOR_DEFAULTTONEAREST 保证返回值为句柄或 null。
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
    // SAFETY: monitor 为非空句柄（上方已校验）；info 为完整初始化的 MONITORINFO，调用期间有效可写。
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
    // SAFETY: hwnd 由调用方保证存活；SetLastError/GetLastError 无句柄参数；GetWindowLongW 返回值只用于错误判定。
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
    // SAFETY: hwnd 由调用方保证存活；SetWindowLongW 同步执行且不保留指针。
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
        // SAFETY: self.hwnd 非空（短路条件）且由本对象持有；IsWindow 只读查询。
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
    fn capabilities(&self) -> WindowCapabilities {
        WINDOWS_WINDOW_CAPABILITIES
    }

    // ── 窗口生命周期 ──────────────────────────────────────

    fn os_show(&mut self) -> Result<()> {
        self.ensure_valid_window("os_show")?;
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；ShowWindow 同步执行。
        unsafe {
            ShowWindow(self.hwnd, SW_SHOWNORMAL);
        }
        Ok(())
    }

    fn os_hide(&mut self) -> Result<()> {
        self.ensure_valid_window("os_hide")?;
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；ShowWindow 同步执行。
        unsafe {
            ShowWindow(self.hwnd, SW_HIDE);
        }
        Ok(())
    }

    fn os_close(&mut self) -> Result<()> {
        self.ensure_valid_window("os_close")?;
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；DestroyWindow 同步销毁且不返回指针。
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
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；PostMessageW 同步投递消息。
        if unsafe { PostMessageW(self.hwnd, WM_CLOSE, 0, 0) } == 0 {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "os_request_close: PostMessageW failed",
            ));
        }
        Ok(())
    }

    // Windows 通过独立非客户区交互 Component 提交移动消息。
    fn os_begin_move_drag(
        // Windows 使用当前 Win32 捕获状态，不消费 Wayland 式授权。
        &mut self,
        // 保留统一签名并明确忽略不可解释身份。
        _pointer_activation: Option<PointerActivationId>,
        // 返回原有 Win32 消息提交结果。
    ) -> Result<()> {
        self.ensure_valid_window("os_begin_move_drag")?;
        // 句柄校验完成后由窄 Component 独占 Win32 消息映射。
        super::window_interaction::begin_move_drag(self.hwnd)
    }

    // Windows 把公共缩放方向映射为对应非客户区 hit-test 消息。
    fn os_begin_resize_drag(
        // 窗口操作只在本次同步调用期间借用自身状态。
        &mut self,
        // 保留 UI 热区声明的精确调整大小方向。
        edge: WindowResizeEdge,
        // Windows 使用当前 Win32 捕获状态，不消费 Wayland 式授权。
        _pointer_activation: Option<PointerActivationId>,
        // 返回 Win32 消息提交结果。
    ) -> Result<()> {
        // 先拒绝已经失效的原生窗口句柄。
        self.ensure_valid_window("os_begin_resize_drag")?;
        // 使用非模态逐指针缩放，让每个 WM_SIZE 都能返回应用消息泵完成布局与呈现。
        super::window_interaction::begin_resize_drag(self.hwnd, edge, &self._binding.resize_drag)
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
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；wide 为存活且 NUL 结尾的 UTF-16 缓冲。
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
        // SAFETY: self.hwnd 已验证存活；rect 为栈上可写结构；monitor 与 rect 均在同步调用期间有效。
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
        // SAFETY: self.hwnd 已验证存活；HWND_TOP 为伪句柄常量，其余参数为同步窗口操作。
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
        // SAFETY: self.hwnd 已验证存活；HWND_BOTTOM 为伪句柄常量。
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
            // SAFETY: hwnd 为本对象持有且已验证存活；尺寸经 DPI 换算非负；SetWindowPos 同步执行。
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
        // SAFETY: self.hwnd 已验证存活；坐标参数为有效窗口位置。
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
        // SAFETY: self.hwnd 已验证存活；样式标志为常量组合；SetWindowPos 同步执行。
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
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；ShowWindow 同步执行。
        unsafe {
            ShowWindow(self.hwnd, SW_MAXIMIZE);
        }
        Ok(())
    }

    fn os_minimize(&mut self) -> Result<()> {
        self.ensure_valid_window("os_minimize")?;
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；ShowWindow 同步执行。
        unsafe {
            ShowWindow(self.hwnd, SW_MINIMIZE);
        }
        Ok(())
    }

    fn os_restore(&mut self) -> Result<()> {
        self.ensure_valid_window("os_restore")?;
        // SAFETY: self.hwnd 已经 ensure_valid_window 验证存活；ShowWindow 同步执行。
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
        // SAFETY: self.hwnd 已验证存活；样式标志为常量组合；SetWindowPos 同步执行。
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
        // SAFETY: self.hwnd 已验证存活；样式标志为常量组合；SetWindowPos 同步执行。
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
            // SAFETY: self.hwnd 已验证存活；placement 为完整初始化的可写结构。
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
            // SAFETY: self.hwnd 已验证存活；monitor.rcMonitor 为存活监视器的工作区数据；SetWindowPos 同步执行。
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
                // SAFETY: self.hwnd 已验证存活；placement 为之前保存的完整窗口布局。
                let _ = unsafe { SetWindowPlacement(self.hwnd, &placement) };
                if style as u32 & WS_VISIBLE == 0 {
                    // SAFETY: self.hwnd 已验证存活；ShowWindow 同步执行。
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
            // SAFETY: self.hwnd 已验证存活；restore.placement 为之前保存的完整窗口布局。
            if unsafe { SetWindowPlacement(self.hwnd, &restore.placement) } == 0 {
                return Err(super::util::windows_diag(
                    Errc::PlatformError,
                    "os_set_fullscreen: SetWindowPlacement failed",
                ));
            }
            if !was_visible {
                // SAFETY: self.hwnd 已验证存活；ShowWindow 同步执行。
                unsafe {
                    ShowWindow(self.hwnd, SW_HIDE);
                }
            }
            // SAFETY: self.hwnd 已验证存活；restore 使用固定窗口标志同步执行。
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
        // SAFETY: self.hwnd 已验证存活；HWND_TOPMOST/HWND_NOTOPMOST 为伪句柄常量。
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
            // SAFETY: self.hwnd 已验证存活；alpha 为 0..=255 的 u8；标志为常量组合。
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
            // SAFETY: self.hwnd 已验证存活；恢复全不透明 alpha 为固定常量。
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
        // SAFETY: self.hwnd 已验证存活；enable 为 BOOL 常量；DragAcceptFiles 同步注册拖放。
        unsafe {
            DragAcceptFiles(self.hwnd, if enable { TRUE } else { FALSE });
        }
        Ok(())
    }

    fn os_resize_notify(&mut self, _width: i32, _height: i32) -> Result<()> {
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

    fn os_occlusion_state(&self) -> WindowOcclusionState {
        WindowOcclusionState::Unknown
    }

    // ── 原生句柄 ──────────────────────────────────────────

    fn native_surface_ptr(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }

    fn client_logical_extent(&self, fallback_width: i32, fallback_height: i32) -> (i32, i32) {
        // Windows 窗口样式或 DPI 正在变化时，以仍有效的客户区查询为准。
        let drawable = crate::native::presentation::graphics::platform::windows::drawable_size(
            self.hwnd,
            fallback_width,
            fallback_height,
        );
        (drawable.logical_width, drawable.logical_height)
    }

    fn native_handle(&self) -> *mut std::ffi::c_void {
        self.hwnd
    }
}

use super::consts::FALSE;
use super::consts::TRUE;
