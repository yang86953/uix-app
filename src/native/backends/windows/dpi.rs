//! Windows 每窗 DPI 与 logical/physical 坐标换算。

#![cfg(windows)]

use std::ffi::c_void;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::HiDpi::GetDpiForWindow;

pub(crate) const BASE_DPI: u32 = 96;

pub(crate) fn valid_dpi(dpi: u32) -> u32 {
    if dpi == 0 {
        BASE_DPI
    } else {
        dpi
    }
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

fn scale_non_negative_extent(value: i32, numerator: u32, denominator: u32) -> i32 {
    if value <= 0 {
        return 0;
    }
    let numerator = i64::from(numerator);
    let denominator = i64::from(denominator.max(1));
    let scaled = (i64::from(value) * numerator + denominator / 2) / denominator;
    scaled.min(i64::from(i32::MAX)) as i32
}
