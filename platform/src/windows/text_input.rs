#![cfg(windows)]
use super::ffi::*;


// ============================================================================
// uix-platform/src/windows/text_input.rs — Windows IME text input (ITextInput)
// ============================================================================

use crate::ITextInput;
use std::ptr;

pub struct WindowsTextInput {
    hwnd: *mut std::ffi::c_void,
}

impl WindowsTextInput {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
        }
    }
    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for WindowsTextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl ITextInput for WindowsTextInput {
    fn start(&mut self) {
        if self.hwnd.is_null() {
            return;
        }
        unsafe {
            ImmAssociateContextEx(self.hwnd, ptr::null_mut(), IACE_DEFAULT);
        }
    }
    fn stop(&mut self) {
        if self.hwnd.is_null() {
            return;
        }
        unsafe {
            ImmAssociateContextEx(self.hwnd, ptr::null_mut(), 0);
        }
    }
}

const IACE_DEFAULT: u32 = 0x0010;

