// ============================================================================
// uix-platform/src/windows/clipboard.rs — Windows clipboard (IClipboard)
// ============================================================================

#![cfg(windows)]

use crate::platform::windows::util::to_utf8;
use crate::platform::windows::util::to_wide;
use crate::platform::IClipboard;
use std::ptr;

pub struct WindowsClipboard {
    hwnd: *mut std::ffi::c_void,
}

impl WindowsClipboard {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
        }
    }

    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for WindowsClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl IClipboard for WindowsClipboard {
    fn text(&self) -> String {
        unsafe {
            if OpenClipboard(self.hwnd) == 0 {
                return String::new();
            }
            let handle = GetClipboardData(CF_UNICODETEXT);
            if handle.is_null() {
                CloseClipboard();
                return String::new();
            }
            let ptr = GlobalLock(handle) as *const u16;
            if ptr.is_null() {
                CloseClipboard();
                return String::new();
            }
            let mut len = 0;
            while *ptr.add(len) != 0 {
                len += 1;
            }
            let result = to_utf8(std::slice::from_raw_parts(ptr, len));
            GlobalUnlock(handle);
            CloseClipboard();
            result
        }
    }

    fn set_text(&mut self, text: &str) {
        unsafe {
            if OpenClipboard(self.hwnd) == 0 {
                return;
            }
            EmptyClipboard();
            let wide = to_wide(text);
            let size = wide.len() * 2;
            let hglobal = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, size);
            if hglobal.is_null() {
                CloseClipboard();
                return;
            }
            let dest = GlobalLock(hglobal) as *mut u16;
            if !dest.is_null() {
                ptr::copy_nonoverlapping(wide.as_ptr(), dest, wide.len());
                GlobalUnlock(hglobal);
            }
            SetClipboardData(CF_UNICODETEXT, hglobal);
            CloseClipboard();
        }
    }

    fn has_text(&self) -> bool {
        unsafe {
            if OpenClipboard(self.hwnd) == 0 {
                return false;
            }
            let formats = [CF_UNICODETEXT, CF_TEXT];
            let fmt = GetPriorityClipboardFormat(formats.as_ptr(), 2);
            CloseClipboard();
            fmt != -1
        }
    }
}

// FFI declarations

const CF_UNICODETEXT: u32 = 13;
const CF_TEXT: u32 = 1;
const GMEM_MOVEABLE: u32 = 0x0002;
const GMEM_ZEROINIT: u32 = 0x0040;

#[link(name = "user32")]
unsafe extern "system" {
    unsafe fn OpenClipboard(hwnd: *mut std::ffi::c_void) -> i32;
    unsafe fn CloseClipboard() -> i32;
    unsafe fn EmptyClipboard() -> i32;
    unsafe fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
    unsafe fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    unsafe fn GetPriorityClipboardFormat(paFormatPriorityList: *const u32, cFormats: i32) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    unsafe fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
    unsafe fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    unsafe fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;
}
