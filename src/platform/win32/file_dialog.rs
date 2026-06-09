// ============================================================================
// uix-platform/src/win32/file_dialog.rs — Win32 file dialog (IFileDialog)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::platform::win32::util::{to_utf8, to_wide};
use crate::platform::IFileDialog;
use std::ptr;

pub struct Win32FileDialog {
    hwnd: *mut std::ffi::c_void,
}

impl Win32FileDialog {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
        }
    }
    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
}

impl Default for Win32FileDialog {
    fn default() -> Self {
        Self::new()
    }
}

impl IFileDialog for Win32FileDialog {
    fn open(&mut self, title: &str, filters: &str) -> Vec<String> {
        let wide_filters = to_wide(filters);
        let mut buf = [0u16; 4096];
        let wide_title = to_wide(title);
        unsafe {
            let mut ofn = OPENFILENAMEW {
                lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: ptr::null_mut(),
                lpstrFilter: wide_filters.as_ptr(),
                lpstrCustomFilter: ptr::null_mut(),
                nMaxCustFilter: 0,
                nFilterIndex: 1,
                lpstrFile: buf.as_mut_ptr(),
                nMaxFile: 4096,
                lpstrFileTitle: ptr::null_mut(),
                nMaxFileTitle: 0,
                lpstrInitialDir: ptr::null(),
                lpstrTitle: wide_title.as_ptr(),
                Flags: OFN_EXPLORER | OFN_FILEMUSTEXIST | OFN_HIDEREADONLY | OFN_ALLOWMULTISELECT,
                nFileOffset: 0,
                nFileExtension: 0,
                lpstrDefExt: ptr::null(),
                lCustData: 0,
                lpfnHook: ptr::null_mut(),
                lpTemplateName: ptr::null(),
                pvReserved: ptr::null_mut(),
                dwReserved: 0,
                FlagsEx: 0,
            };
            let result = GetOpenFileNameW(&mut ofn);
            if result == 0 {
                return Vec::new();
            }
            let wide_str = &buf[..];
            let null_pos = wide_str.iter().position(|&c| c == 0).unwrap_or(0);
            if null_pos == 0 {
                return Vec::new();
            }
            let dir = to_utf8(&wide_str[..null_pos]);
            if dir.is_empty() {
                return Vec::new();
            }
            let remaining = &wide_str[(null_pos + 1)..];
            let second_null = remaining.iter().position(|&c| c == 0).unwrap_or(0);
            if second_null == 0 || remaining[0] == 0 {
                return vec![dir];
            }
            let mut result = Vec::new();
            let mut pos = 0;
            loop {
                if pos >= remaining.len() || remaining[pos] == 0 {
                    break;
                }
                let end = remaining[pos..]
                    .iter()
                    .position(|&c| c == 0)
                    .unwrap_or(remaining.len() - pos);
                let file_name = to_utf8(&remaining[pos..pos + end]);
                if file_name.is_empty() {
                    break;
                }
                result.push(format!("{}\\{}", dir, file_name));
                pos = pos + end + 1;
            }
            result
        }
    }

    fn save(&mut self, title: &str, filters: &str) -> String {
        let wide_filters = to_wide(filters);
        let mut buf = [0u16; 4096];
        let wide_title = to_wide(title);
        unsafe {
            let mut ofn = OPENFILENAMEW {
                lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
                hwndOwner: self.hwnd,
                hInstance: ptr::null_mut(),
                lpstrFilter: wide_filters.as_ptr(),
                lpstrCustomFilter: ptr::null_mut(),
                nMaxCustFilter: 0,
                nFilterIndex: 1,
                lpstrFile: buf.as_mut_ptr(),
                nMaxFile: 4096,
                lpstrFileTitle: ptr::null_mut(),
                nMaxFileTitle: 0,
                lpstrInitialDir: ptr::null(),
                lpstrTitle: wide_title.as_ptr(),
                Flags: OFN_EXPLORER | OFN_PATHMUSTEXIST | OFN_HIDEREADONLY | OFN_OVERWRITEPROMPT,
                nFileOffset: 0,
                nFileExtension: 0,
                lpstrDefExt: ptr::null(),
                lCustData: 0,
                lpfnHook: ptr::null_mut(),
                lpTemplateName: ptr::null(),
                pvReserved: ptr::null_mut(),
                dwReserved: 0,
                FlagsEx: 0,
            };
            let result = GetSaveFileNameW(&mut ofn);
            if result == 0 {
                return String::new();
            }
            to_utf8(&buf)
        }
    }

    fn open_folder(&mut self, _title: &str) -> String {
        String::new()
    }
}

// ── FFI ──

#[repr(C)]
struct OPENFILENAMEW {
    lStructSize: u32,
    hwndOwner: *mut std::ffi::c_void,
    hInstance: *mut std::ffi::c_void,
    lpstrFilter: *const u16,
    lpstrCustomFilter: *mut u16,
    nMaxCustFilter: u32,
    nFilterIndex: u32,
    lpstrFile: *mut u16,
    nMaxFile: u32,
    lpstrFileTitle: *mut u16,
    nMaxFileTitle: u32,
    lpstrInitialDir: *const u16,
    lpstrTitle: *const u16,
    Flags: u32,
    nFileOffset: u16,
    nFileExtension: u16,
    lpstrDefExt: *const u16,
    lCustData: isize,
    lpfnHook: *mut std::ffi::c_void,
    lpTemplateName: *const u16,
    pvReserved: *mut std::ffi::c_void,
    dwReserved: u32,
    FlagsEx: u32,
}

const OFN_EXPLORER: u32 = 0x00080000;
const OFN_FILEMUSTEXIST: u32 = 0x00001000;
const OFN_HIDEREADONLY: u32 = 0x00000004;
const OFN_ALLOWMULTISELECT: u32 = 0x00000200;
const OFN_PATHMUSTEXIST: u32 = 0x00000800;
const OFN_OVERWRITEPROMPT: u32 = 0x00000002;

#[link(name = "comdlg32")]
extern "system" {
    fn GetOpenFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
    fn GetSaveFileNameW(lpofn: *mut OPENFILENAMEW) -> i32;
}
