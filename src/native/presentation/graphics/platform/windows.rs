//! Win32 surface helpers for graphics API platform adapters.

#![cfg(windows)]
#![allow(nonstandard_style)]

use std::ffi::c_void;

use crate::native::backends::windows::dpi::{
    dpi_for_window, logical_extent_to_physical, physical_extent_to_logical,
};

// These helpers belong to the shared native surface path, which is compiled
// even when every optional native backend feature is disabled.

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
    #[cfg(all(test, feature = "opengles"))]
    fn GetDC(hwnd: *mut c_void) -> *mut c_void;
    #[cfg(all(test, feature = "opengles"))]
    fn ReleaseDC(hwnd: *mut c_void, hdc: *mut c_void) -> i32;
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

fn physical_client_size(hwnd: *mut c_void) -> Option<(i32, i32)> {
    unsafe {
        query_client_rect(hwnd).map(|rect| {
            (
                (rect.right - rect.left).max(1),
                (rect.bottom - rect.top).max(1),
            )
        })
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
        width: logical_extent_to_physical(logical_width, dpi as u32).max(1),
        height: logical_extent_to_physical(logical_height, dpi as u32).max(1),
    }
}

pub(crate) fn drawable_size_from_client_pixels(
    physical_width: i32,
    physical_height: i32,
    dpi: u32,
) -> DrawableSize {
    let width = physical_width.max(1);
    let height = physical_height.max(1);
    DrawableSize {
        logical_width: physical_extent_to_logical(width, dpi).max(1),
        logical_height: physical_extent_to_logical(height, dpi).max(1),
        width,
        height,
    }
}

/// Computes logical client and physical drawable extents from an already-held
/// HDC. WGL owns an HDC for its lifetime and therefore uses this variant.
pub(crate) fn drawable_size_from_hdc(
    hwnd: *mut c_void,
    _hdc: *mut c_void,
    fallback_w: i32,
    fallback_h: i32,
) -> DrawableSize {
    let dpi = dpi_for_window(hwnd);
    physical_client_size(hwnd).map_or_else(
        || drawable_size_from_dpi(fallback_w, fallback_h, dpi as i32),
        |(width, height)| drawable_size_from_client_pixels(width, height, dpi),
    )
}

/// Computes the shared Windows graphics drawable extent. D3D contexts acquire
/// a short-lived HDC; callers that already own one use
/// [`drawable_size_from_hdc`] instead.
pub(crate) fn drawable_size(hwnd: *mut c_void, fallback_w: i32, fallback_h: i32) -> DrawableSize {
    drawable_size_from_hdc(hwnd, std::ptr::null_mut(), fallback_w, fallback_h)
}

#[cfg(all(test, feature = "opengles"))]
pub(crate) unsafe fn device_context(hwnd: *mut c_void) -> *mut c_void {
    GetDC(hwnd)
}

#[cfg(all(test, feature = "opengles"))]
pub(crate) unsafe fn release_device_context(hwnd: *mut c_void, hdc: *mut c_void) {
    let _ = ReleaseDC(hwnd, hdc);
}

/// Checked release used by a `Result`-returning graphics lifecycle path.
/// Callers retain the HDC on failure so teardown can report and retry rather
/// than silently discarding the last native error.
#[cfg(all(test, feature = "opengles"))]
pub(crate) unsafe fn release_device_context_checked(hwnd: *mut c_void, hdc: *mut c_void) -> bool {
    ReleaseDC(hwnd, hdc) != 0
}
