//! Windows 每窗 DPI 与 logical/physical 坐标换算。

#![cfg(windows)]

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForSystem,
    SetThreadDpiAwarenessContext,
};

use super::bindings::RECT;
use super::consts::{FALSE, SWP_NOACTIVATE, SWP_NOZORDER};
use super::ffi::{AdjustWindowRectExForDpi, SetWindowPos};
use super::util::windows_diag;
use crate::native::{Errc, Error};

pub(crate) const BASE_DPI: u32 = 96;

pub(crate) fn valid_dpi(dpi: u32) -> u32 {
    if dpi == 0 { BASE_DPI } else { dpi }
}

/// 把非负 logical extent 按 DPI 就近取整为 physical pixels。
pub(crate) fn logical_extent_to_physical(value: i32, dpi: u32) -> i32 {
    scale_non_negative_extent(value, valid_dpi(dpi), BASE_DPI)
}

/// 把非负 physical extent 按 DPI 就近取整为 logical pixels。
pub(crate) fn physical_extent_to_logical(value: i32, dpi: u32) -> i32 {
    scale_non_negative_extent(value, BASE_DPI, valid_dpi(dpi))
}

/// 把客户区 physical point 换算为 UI 使用的 logical point。
pub(crate) fn physical_point_to_logical(value: i32, dpi: u32) -> f32 {
    value as f32 * BASE_DPI as f32 / valid_dpi(dpi) as f32
}

/// 查询 HWND 自身的有效 DPI；无效句柄或 API 失败时保守回退到 96。
pub(crate) fn dpi_for_window(hwnd: *mut c_void) -> u32 {
    if hwnd.is_null() {
        return BASE_DPI;
    }
    // SAFETY: GetDpiForWindow 只读取 HWND；无效句柄会返回 0，再由 valid_dpi 回退。
    valid_dpi(unsafe { GetDpiForWindow(HWND(hwnd)) })
}

pub(crate) fn dpi_for_system() -> u32 {
    // SAFETY: GetDpiForSystem 不解引用应用指针，返回值按当前线程 awareness 解释。
    valid_dpi(unsafe { GetDpiForSystem() })
}

/// 计算指定 DPI 下容纳 logical client extent 所需的原生外窗尺寸。
pub(crate) fn outer_size_for_logical_client(
    logical_width: i32,
    logical_height: i32,
    style: u32,
    ex_style: u32,
    dpi: u32,
) -> Result<(i32, i32), Error> {
    if logical_width <= 0 || logical_height <= 0 {
        return Err(Error::new(
            Errc::InvalidArgument,
            format!("Windows client extent must be positive, got {logical_width}x{logical_height}"),
        ));
    }
    let dpi = valid_dpi(dpi);
    let physical_width = logical_extent_to_physical(logical_width, dpi);
    let physical_height = logical_extent_to_physical(logical_height, dpi);
    // 自定义标题栏扩展客户区后，外窗与客户区同尺寸；再走 AdjustWindowRect
    // 会把已“吃掉”的 THICKFRAME 边框重复计入，导致窗口偏大。
    if super::custom_chrome::outer_matches_client_when_extended(style) {
        return Ok((physical_width, physical_height));
    }
    let mut rect = RECT {
        left: 0,
        top: 0,
        right: physical_width,
        bottom: physical_height,
    };
    // SAFETY: rect 在同步调用期间有效，style/ex_style 来自同一 Win32 窗口配置。
    if unsafe { AdjustWindowRectExForDpi(&mut rect, style, FALSE, ex_style, dpi) } == 0 {
        return Err(windows_diag(
            Errc::PlatformError,
            "AdjustWindowRectExForDpi failed",
        ));
    }
    Ok((rect.right - rect.left, rect.bottom - rect.top))
}

/// 应用 `WM_DPICHANGED` 提供的 physical 外窗矩形。
///
/// # Safety
///
/// `suggested` 必须指向当前同步消息调用期间有效的 Win32 `RECT`。
pub(super) unsafe fn apply_suggested_window_rect(
    hwnd: *mut c_void,
    suggested: *const RECT,
) -> Result<(), Error> {
    if hwnd.is_null() || suggested.is_null() {
        return Err(Error::new(
            Errc::InvalidArgument,
            "WM_DPICHANGED requires a valid HWND and suggested RECT",
        ));
    }
    // SAFETY: 有效期由调用方保证；read_unaligned 不额外要求指针对齐。
    let rect = unsafe { std::ptr::read_unaligned(suggested) };
    let width = rect.right.saturating_sub(rect.left);
    let height = rect.bottom.saturating_sub(rect.top);
    if width <= 0 || height <= 0 {
        return Err(Error::new(
            Errc::InvalidArgument,
            format!("WM_DPICHANGED suggested an invalid window extent {width}x{height}"),
        ));
    }
    // SAFETY: HWND 与矩形已校验，调用不保留 Rust 指针。
    if unsafe {
        SetWindowPos(
            hwnd,
            std::ptr::null_mut(),
            rect.left,
            rect.top,
            width,
            height,
            SWP_NOZORDER | SWP_NOACTIVATE,
        )
    } == 0
    {
        return Err(windows_diag(
            Errc::PlatformError,
            "WM_DPICHANGED SetWindowPos failed",
        ));
    }
    Ok(())
}

/// 临时把当前线程切到 Per-Monitor V2；调用方必须显式 finish 以保留 typed 失败。
pub(crate) struct PerMonitorV2Scope {
    previous: Option<DPI_AWARENESS_CONTEXT>,
}

impl PerMonitorV2Scope {
    pub(crate) fn enter() -> Result<Self, Error> {
        // SAFETY: 只改变当前线程，返回的 previous 由 finish/Drop 在同线程恢复。
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.0.is_null() {
            return Err(windows_diag(
                Errc::PlatformError,
                "SetThreadDpiAwarenessContext(PER_MONITOR_AWARE_V2) failed",
            ));
        }
        Ok(Self {
            previous: Some(previous),
        })
    }

    pub(crate) fn finish(mut self) -> Result<(), Error> {
        let Some(previous) = self.previous.take() else {
            return Ok(());
        };
        restore_thread_context(previous)
    }
}

impl Drop for PerMonitorV2Scope {
    fn drop(&mut self) {
        let Some(previous) = self.previous.take() else {
            return;
        };
        if let Err(error) = restore_thread_context(previous) {
            // teardown 边界的线程上下文恢复失败只保留日志：此刻窗口已在
            // 关闭路径，报告化无额外消费者。
            tracing::error!("{}", error.short_what());
        }
    }
}

pub(crate) fn with_per_monitor_v2<T>(
    operation: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    let scope = PerMonitorV2Scope::enter()?;
    let operation_result = operation();
    let restore_result = scope.finish();
    match (operation_result, restore_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Ok(_), Err(restore)) => Err(restore),
        (Err(operation), Ok(())) => Err(operation),
        (Err(operation), Err(restore)) => {
            let code = operation.code();
            let message = format!(
                "{}; DPI context restore also failed: {}",
                operation.message(),
                restore.message()
            );
            Err(Error::new(code, message).with_source(operation))
        }
    }
}

fn restore_thread_context(previous: DPI_AWARENESS_CONTEXT) -> Result<(), Error> {
    // SAFETY: previous 由当前线程刚才的 SetThreadDpiAwarenessContext 返回。
    let replaced = unsafe { SetThreadDpiAwarenessContext(previous) };
    if replaced.0.is_null() {
        Err(windows_diag(
            Errc::PlatformError,
            "restoring the previous thread DPI awareness context failed",
        ))
    } else {
        Ok(())
    }
}

fn scale_non_negative_extent(value: i32, numerator: u32, denominator: u32) -> i32 {
    if value <= 0 {
        return 0;
    }
    let numerator = i64::from(numerator);
    let denominator = i64::from(denominator.max(1));
    let scaled = (i64::from(value) * numerator + denominator / 2) / denominator;
    scaled.min(i64::from(i32::MAX)) as i32
}
