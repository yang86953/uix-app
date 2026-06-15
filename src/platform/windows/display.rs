// ============================================================================
// uix-platform/src/windows/display.rs — Windows display info (IDisplay)
// ============================================================================

#![cfg(windows)]

use super::ffi::*;
use crate::platform::types::Rect;
use crate::platform::{DisplayInfo, IDisplay};
use std::ptr;

pub struct WindowsDisplay;

impl WindowsDisplay {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl IDisplay for WindowsDisplay {
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
