//! Win32 surface helpers for graphics API platform adapters.

#![cfg(windows)]
#![allow(nonstandard_style)]

use std::ffi::c_void;

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

pub(crate) fn client_size(hwnd: *mut c_void, fallback_w: i32, fallback_h: i32) -> (i32, i32) {
    unsafe {
        let mut rect = Rect {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if GetClientRect(hwnd, &mut rect) == 0 {
            return (fallback_w.max(1), fallback_h.max(1));
        }
        (
            (rect.right - rect.left).max(1),
            (rect.bottom - rect.top).max(1),
        )
    }
}

pub(crate) unsafe fn device_context(hwnd: *mut c_void) -> *mut c_void {
    GetDC(hwnd)
}

pub(crate) unsafe fn release_device_context(hwnd: *mut c_void, hdc: *mut c_void) {
    let _ = ReleaseDC(hwnd, hdc);
}
