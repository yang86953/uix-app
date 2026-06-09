// ============================================================================
// uix-platform/src/win32/display.rs — Win32 display info (IDisplay)
// ============================================================================

#![cfg(windows)]

use crate::platform::types::Rect;
use crate::platform::{DisplayInfo, IDisplay};
use std::ptr;

pub struct Win32Display;

impl Win32Display {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Win32Display {
    fn default() -> Self {
        Self::new()
    }
}

impl IDisplay for Win32Display {
    fn dpi_scale(&self) -> f32 {
        unsafe {
            let hdc = GetDC(ptr::null_mut());
            if hdc.is_null() {
                return 1.0;
            }
            let dpi = GetDeviceCaps(hdc, LOGPIXELSX);
            ReleaseDC(ptr::null_mut(), hdc);
            dpi as f32 / 96.0
        }
    }

    fn is_dark_mode(&self) -> bool {
        false
    }

    fn count(&self) -> i32 {
        1
    }

    fn info(&self, _index: i32) -> DisplayInfo {
        unsafe {
            let w = GetSystemMetrics(SM_CXSCREEN);
            let h = GetSystemMetrics(SM_CYSCREEN);
            let dpi_scale = self.dpi_scale();
            DisplayInfo {
                bounds: Rect::new(0.0, 0.0, w as f32, h as f32),
                dpi_scale,
                is_primary: true,
            }
        }
    }
}

const LOGPIXELSX: i32 = 88;
const SM_CXSCREEN: i32 = 0;
const SM_CYSCREEN: i32 = 1;

#[link(name = "gdi32")]
extern "system" {
    fn GetDeviceCaps(hdc: *mut std::ffi::c_void, nIndex: i32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    fn GetSystemMetrics(nIndex: i32) -> i32;
}
