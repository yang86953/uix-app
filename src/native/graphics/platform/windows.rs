//! Win32 surface helpers for graphics API platform adapters.

#![cfg(windows)]
#![allow(nonstandard_style)]

use std::ffi::c_void;

const LOGPIXELSX: i32 = 88;

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Rect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[link(name = "user32")]
extern "system" {
    fn GetClientRect(hwnd: *mut c_void, lp_rect: *mut Rect) -> i32;
    fn GetDC(hwnd: *mut c_void) -> *mut c_void;
    fn ReleaseDC(hwnd: *mut c_void, hdc: *mut c_void) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn GetDeviceCaps(hdc: *mut c_void, index: i32) -> i32;
}

/// Logical client extent and the physical drawable extent derived from the
/// same monitor DPI. All Windows graphics APIs consume this value so caps,
/// swapchain allocation, and raster scissor coordinates agree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawableSize {
    pub(crate) logical_width: i32,
    pub(crate) logical_height: i32,
    pub(crate) width: i32,
    pub(crate) height: i32,
}

pub(crate) unsafe fn query_client_rect(hwnd: *mut c_void) -> Option<Rect> {
    let mut rect = Rect {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    if GetClientRect(hwnd, &mut rect) == 0 {
        None
    } else {
        Some(rect)
    }
}

fn logical_size(hwnd: *mut c_void, fallback_w: i32, fallback_h: i32) -> (i32, i32) {
    unsafe {
        let Some(rect) = query_client_rect(hwnd) else {
            return (fallback_w.max(1), fallback_h.max(1));
        };
        (
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
        )
    }
}

pub(crate) fn drawable_size_from_dpi(
    logical_width: i32,
    logical_height: i32,
    dpi: i32,
) -> DrawableSize {
    let logical_width = logical_width.max(1);
    let logical_height = logical_height.max(1);
    let dpi = dpi.max(96);
    DrawableSize {
        logical_width,
        logical_height,
        width: ((logical_width as i64 * dpi as i64 + 48) / 96).max(1) as i32,
        height: ((logical_height as i64 * dpi as i64 + 48) / 96).max(1) as i32,
    }
}

/// Computes logical client and physical drawable extents from an already-held
/// HDC. WGL owns an HDC for its lifetime and therefore uses this variant.
pub(crate) fn drawable_size_from_hdc(
    hwnd: *mut c_void,
    hdc: *mut c_void,
    fallback_w: i32,
    fallback_h: i32,
) -> DrawableSize {
    let (logical_width, logical_height) = logical_size(hwnd, fallback_w, fallback_h);
    let dpi = if hdc.is_null() {
        96
    } else {
        unsafe { GetDeviceCaps(hdc, LOGPIXELSX) }.max(96)
    };
    drawable_size_from_dpi(logical_width, logical_height, dpi)
}

/// Computes the shared Windows graphics drawable extent. D3D contexts acquire
/// a short-lived HDC; callers that already own one use
/// [`drawable_size_from_hdc`] instead.
pub(crate) fn drawable_size(hwnd: *mut c_void, fallback_w: i32, fallback_h: i32) -> DrawableSize {
    unsafe {
        let hdc = GetDC(hwnd);
        let size = drawable_size_from_hdc(hwnd, hdc, fallback_w, fallback_h);
        if !hdc.is_null() {
            let _ = ReleaseDC(hwnd, hdc);
        }
        size
    }
}

pub(crate) unsafe fn device_context(hwnd: *mut c_void) -> *mut c_void {
    GetDC(hwnd)
}

pub(crate) unsafe fn release_device_context(hwnd: *mut c_void, hdc: *mut c_void) {
    let _ = ReleaseDC(hwnd, hdc);
}

/// Checked release used by a `Result`-returning graphics lifecycle path.
/// Callers retain the HDC on failure so teardown can report and retry rather
/// than silently discarding the last native error.
pub(crate) unsafe fn release_device_context_checked(hwnd: *mut c_void, hdc: *mut c_void) -> bool {
    ReleaseDC(hwnd, hdc) != 0
}

