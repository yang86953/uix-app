//! 自定义标题栏客户区扩展：去掉系统标题栏后仍保留 `WS_THICKFRAME`，
//! 但默认非客户区缩放边框会在 HWND 四周留下未绘制空隙（桌面/宿主背景透出）。
//! 通过 `WM_NCCALCSIZE` 把客户区扩到外窗，并用 `WM_NCHITTEST` 在边缘恢复缩放命中。
//!
//! 客户区扩满后 DWM 不再有标准非客户区可绘阴影/圆角；需
//! `DwmExtendFrameIntoClientArea`（1px 底边）与 `DWMWA_WINDOW_CORNER_PREFERENCE` 恢复。

#![cfg(windows)]

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{
    DWM_WINDOW_CORNER_PREFERENCE, DWMNCRENDERINGPOLICY, DWMNCRP_ENABLED, DWMNCRP_USEWINDOWSTYLE,
    DWMWA_NCRENDERING_POLICY, DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND, DWMWCP_ROUND,
    DwmExtendFrameIntoClientArea, DwmSetWindowAttribute,
};
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::HiDpi::GetSystemMetricsForDpi;
use windows::Win32::UI::WindowsAndMessaging::{SM_CXFRAME, SM_CXPADDEDBORDER, SM_CYFRAME};

use super::bindings::{MONITORINFO, RECT};
use super::consts::{
    GWL_STYLE, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCLIENT, HTLEFT, HTRIGHT, HTTOP, HTTOPLEFT,
    HTTOPRIGHT, MONITOR_DEFAULTTONEAREST, WS_CAPTION, WS_MAXIMIZE, WS_THICKFRAME,
};
use super::dpi::{BASE_DPI, dpi_for_window, logical_extent_to_physical};
use super::ffi::{GetMonitorInfoW, GetWindowRect, IsZoomed, MonitorFromWindow};
use crate::native::{Errc, Error, Result};

#[repr(C)]
pub(crate) struct NcCalcSizeParams {
    pub rgrc: [RECT; 3],
    pub lppos: *mut c_void,
}

/// 无系统标题栏、仍保留粗边框时，客户区应铺满外窗。
pub(crate) fn uses_extended_client(style: u32) -> bool {
    (style & WS_CAPTION) == 0 && (style & WS_THICKFRAME) != 0
}

pub(crate) fn window_style(hwnd: *mut c_void) -> Result<u32> {
    if hwnd.is_null() {
        return Ok(0);
    }
    super::window_ops::get_window_long_checked(
        hwnd,
        GWL_STYLE,
        "custom chrome: GetWindowLongW(GWL_STYLE) failed",
    )
    .map(|style| style as u32)
}

/// 最大化判定需覆盖 `WM_NCCALCSIZE` 过渡期（此时 `WindowState.maximized` 可能尚未更新）。
pub(crate) fn is_effectively_maximized(
    hwnd: *mut c_void,
    style: u32,
    state_maximized: bool,
) -> bool {
    if state_maximized {
        return true;
    }
    if style & WS_MAXIMIZE != 0 {
        return true;
    }
    if hwnd.is_null() {
        return false;
    }
    // SAFETY: IsZoomed 只查询 HWND 最大化状态。
    unsafe { IsZoomed(hwnd) != 0 }
}

/// 还原后强制重算非客户区；否则最大化期的边框内缩会残留，客户区小于外窗。
pub(crate) fn refresh_extended_client_frame(hwnd: *mut c_void, style: u32) -> Result<()> {
    if hwnd.is_null() || !uses_extended_client(style) {
        return Ok(());
    }
    use super::consts::{SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER};
    use super::ffi::SetWindowPos;
    // SAFETY: 仅触发帧重算；不改位置/尺寸/Z 序。
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
        )
    } == 0
    {
        return Err(super::util::windows_diag(
            Errc::PlatformError,
            "custom chrome: SetWindowPos(FRAMECHANGED) failed",
        ));
    }
    Ok(())
}

/// 扩展客户区下恢复 DWM 阴影与 Win11 圆角；最大化时关闭圆角。
pub(crate) fn apply_dwm_frame_effects(
    hwnd: *mut c_void,
    style: u32,
    maximized: bool,
) -> Result<()> {
    if hwnd.is_null() {
        return Ok(());
    }
    let handle = HWND(hwnd);
    if !uses_extended_client(style) {
        let zero = MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 0,
        };
        // SAFETY: HWND 来自当前窗口；清掉扩展边距，交还系统默认帧合成。
        unsafe { DwmExtendFrameIntoClientArea(handle, &zero) }.map_err(|error| {
            dwm_failure(
                "custom chrome: DwmExtendFrameIntoClientArea(reset) failed",
                error,
            )
        })?;
        set_nc_rendering_policy(handle, DWMNCRP_USEWINDOWSTYLE)?;
        return Ok(());
    }
    // 最大化窗口无阴影需求：清零扩展边距，避免 DWM 在客户区底部残留 1px 边框线；
    // 还原态用 1px 底边让 DWM 继续画阴影，又不会露出标准边框。
    let margins = if maximized {
        MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 0,
        }
    } else {
        MARGINS {
            cxLeftWidth: 0,
            cxRightWidth: 0,
            cyTopHeight: 0,
            cyBottomHeight: 1,
        }
    };
    // SAFETY: 同步 DWM 调用；margins 在调用期间有效。
    unsafe { DwmExtendFrameIntoClientArea(handle, &margins) }.map_err(|error| {
        dwm_failure(
            "custom chrome: DwmExtendFrameIntoClientArea(shadow) failed",
            error,
        )
    })?;
    // 自绘标题栏移除了 WS_CAPTION，显式启用 DWM 非客户区合成，避免阴影随窗口样式被关闭。
    set_nc_rendering_policy(handle, DWMNCRP_ENABLED)?;
    let preference: DWM_WINDOW_CORNER_PREFERENCE = if maximized {
        DWMWCP_DONOTROUND
    } else {
        DWMWCP_ROUND
    };
    // SAFETY: attribute 缓冲与枚举同寿，长度匹配 DWMWA_WINDOW_CORNER_PREFERENCE。
    unsafe {
        DwmSetWindowAttribute(
            handle,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            (&preference as *const DWM_WINDOW_CORNER_PREFERENCE).cast::<c_void>(),
            std::mem::size_of_val(&preference) as u32,
        )
    }
    .map_err(|error| dwm_failure("custom chrome: DwmSetWindowAttribute(corner) failed", error))?;
    Ok(())
}

// 将自绘/系统标题栏事实映射为唯一 DWM 非客户区渲染策略。
fn set_nc_rendering_policy(hwnd: HWND, policy: DWMNCRENDERINGPOLICY) -> Result<()> {
    // SAFETY: attribute 缓冲与枚举同寿，长度匹配 DWMWA_NCRENDERING_POLICY。
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            (&policy as *const DWMNCRENDERINGPOLICY).cast::<c_void>(),
            std::mem::size_of_val(&policy) as u32,
        )
    }
    .map_err(|error| {
        dwm_failure(
            "custom chrome: DwmSetWindowAttribute(non-client rendering) failed",
            error,
        )
    })
}

fn dwm_failure(operation: &str, error: windows::core::Error) -> Error {
    Error::new(Errc::PlatformError, format!("{operation}: {error}"))
}

/// 查询窗口所在显示器的工作区（排除任务栏等系统区域），失败时返回 None。
fn monitor_work_area(hwnd: *mut c_void) -> Option<RECT> {
    if hwnd.is_null() {
        return None;
    }
    // SAFETY: hwnd 属于当前同步窗口消息；返回的显示器句柄仅用于本次查询。
    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor.is_null() {
        return None;
    }
    let mut info = MONITORINFO {
        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
        rcMonitor: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        rcWork: RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        dwFlags: 0,
    };
    // SAFETY: info 在调用期间有效可写。
    if unsafe { GetMonitorInfoW(monitor, &mut info) } == 0 {
        return None;
    }
    Some(info.rcWork)
}

/// 当前 DPI 下单侧缩放边框厚度（physical pixels）。
pub(crate) fn resize_border_thickness(hwnd: *mut c_void) -> i32 {
    let dpi = dpi_for_window(hwnd);
    // SAFETY: GetSystemMetricsForDpi 只按指标与 DPI 返回系统度量。
    let frame_x = unsafe { GetSystemMetricsForDpi(SM_CXFRAME, dpi) };
    let frame_y = unsafe { GetSystemMetricsForDpi(SM_CYFRAME, dpi) };
    let padded = unsafe { GetSystemMetricsForDpi(SM_CXPADDEDBORDER, dpi) };
    let thickness = frame_x.max(frame_y).saturating_add(padded);
    if thickness > 0 {
        thickness
    } else {
        // 极端环境下度量失败时，按 96DPI 的常见 8px 边框回退并随 DPI 缩放。
        logical_extent_to_physical(8, dpi.max(BASE_DPI))
    }
}

/// 自定义标题栏下外窗尺寸等于目标客户区（客户区已扩满外窗）。
pub(crate) fn outer_matches_client_when_extended(style: u32) -> bool {
    uses_extended_client(style)
}

/// 处理 `WM_NCCALCSIZE`：扩展客户区；最大化时对齐到显示器工作区，
/// 避免露出 DWM 非客户区边框或遮住任务栏。
///
/// # Safety
/// `lparam` 必须指向当前同步消息期间有效的 `RECT` 或 [`NcCalcSizeParams`]。
pub(crate) unsafe fn handle_nc_calc_size(
    hwnd: *mut c_void,
    style: u32,
    wparam: usize,
    lparam: isize,
    maximized: bool,
) -> Option<isize> {
    // SAFETY: 调用者保证 lparam 指向当前 WM_NCCALCSIZE 消息的结构；本块只在消息同步处理期间读写该内存。
    unsafe {
        if lparam == 0 || !uses_extended_client(style) {
            return None;
        }
        if wparam == 0 {
            // lParam 为建议窗口矩形；返回 0 表示客户区 = 该矩形。
            return Some(0);
        }
        let params = lparam as *mut NcCalcSizeParams;
        if params.is_null() {
            return None;
        }
        if maximized {
            // 最大化时系统给出的外窗矩形比可见工作区大（含屏幕外的缩放边框）：
            // 直接铺满会遮住任务栏，内缩四边又会让窗口顶部/底部露出 DWM 非客户区
            // （浅色主题下即 1px 白线）。正确做法是把客户区对齐到所在显示器的工作区。
            if let Some(work) = monitor_work_area(hwnd) {
                (*params).rgrc[0] = work;
            } else {
                // 工作区查询失败时回退到内缩边框，保证不遮任务栏。
                let border = resize_border_thickness(hwnd);
                let rect = &mut (*params).rgrc[0];
                rect.left = rect.left.saturating_add(border);
                rect.top = rect.top.saturating_add(border);
                rect.right = rect.right.saturating_sub(border);
                rect.bottom = rect.bottom.saturating_sub(border);
            }
        }
        Some(0)
    }
}

/// 最大化时把外窗矩形钉到所在显示器工作区，消除底部可见的非客户区黑条。
///
/// Win32 对保留 `WS_THICKFRAME` 的窗口最大化会把外窗四边各扩一个缩放边框
/// （约 8px）：上/左/右超出屏幕不可见，底部边框却落在任务栏上缘的屏幕
/// 可见区内；客户区经 `WM_NCCALCSIZE` 对齐回工作区后，这条底部非客户区
/// 没有任何绘制方（无系统标题栏的窗口 DWM 不绘制非客户区），呈现为
/// 任务栏上方一条黑色横条。把外窗收缩到工作区后客户区铺满外窗。
pub(crate) fn snap_maximized_frame_to_work_area(hwnd: *mut c_void) -> Result<()> {
    if hwnd.is_null() {
        return Ok(());
    }
    // 只服务扩展客户区（无系统标题栏 + WS_THICKFRAME）窗口：标准窗口的
    // 最大化边框由系统非客户区绘制在屏幕外，钉窗口反而会让边框退回屏内。
    let style = window_style(hwnd)?;
    if !uses_extended_client(style) {
        return Ok(());
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    // SAFETY: hwnd 属于当前同步窗口消息；rect 在调用期间有效可写。
    if unsafe { GetWindowRect(hwnd, &mut rect) } == 0 {
        return Err(super::util::windows_diag(
            Errc::PlatformError,
            "custom chrome: snap-maximized GetWindowRect failed",
        ));
    }
    // 工作区不可得时保留 Win32 默认最大化几何，不放大失败面。
    let Some(work) = monitor_work_area(hwnd) else {
        return Ok(());
    };
    // 外窗已与工作区一致时跳过；客户区尺寸未变时 Win32 不再重入 WM_SIZE。
    if rect.left == work.left
        && rect.top == work.top
        && rect.right == work.right
        && rect.bottom == work.bottom
    {
        return Ok(());
    }
    let width = work.right.saturating_sub(work.left);
    let height = work.bottom.saturating_sub(work.top);
    use super::consts::{SWP_NOACTIVATE, SWP_NOZORDER};
    use super::ffi::SetWindowPos;
    // SAFETY: hwnd 属于当前同步消息的窗口；工作区矩形为显示器有效坐标。
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            work.left,
            work.top,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } == 0
    {
        return Err(super::util::windows_diag(
            Errc::PlatformError,
            "custom chrome: snap-maximized SetWindowPos failed",
        ));
    }
    Ok(())
}

pub(crate) fn screen_point_from_lparam(lparam: isize) -> (i32, i32) {
    let x = (lparam & 0xFFFF) as i16 as i32;
    let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;
    (x, y)
}

/// 在扩展客户区模式下，把外窗边缘命中映射回标准 `HT*` 缩放结果。
///
/// # Safety
/// `hwnd` 必须是当前消息所属窗口。
pub(crate) unsafe fn handle_nc_hit_test(
    hwnd: *mut c_void,
    style: u32,
    lparam: isize,
    resizable: bool,
) -> Result<Option<isize>> {
    // SAFETY: 调用者保证 HWND 属于当前消息；本块只查询窗口矩形并按值解析 lparam 中的屏幕坐标。
    unsafe {
        if !uses_extended_client(style) {
            return Ok(None);
        }
        if !resizable {
            return Ok(Some(HTCLIENT as isize));
        }
        let mut window_rect = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetWindowRect(hwnd, &mut window_rect) == 0 {
            return Err(super::util::windows_diag(
                Errc::PlatformError,
                "WM_NCHITTEST GetWindowRect failed",
            ));
        }
        let (x, y) = screen_point_from_lparam(lparam);
        let border = resize_border_thickness(hwnd).max(1);
        let left = x - window_rect.left < border;
        let right = window_rect.right - x <= border;
        let top = y - window_rect.top < border;
        let bottom = window_rect.bottom - y <= border;
        let hit = match (left, right, top, bottom) {
            (true, false, true, false) => HTTOPLEFT,
            (false, true, true, false) => HTTOPRIGHT,
            (true, false, false, true) => HTBOTTOMLEFT,
            (false, true, false, true) => HTBOTTOMRIGHT,
            (true, false, false, false) => HTLEFT,
            (false, true, false, false) => HTRIGHT,
            (false, false, true, false) => HTTOP,
            (false, false, false, true) => HTBOTTOM,
            _ => HTCLIENT,
        };
        Ok(Some(hit as isize))
    }
}
