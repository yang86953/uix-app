// ============================================================================
// uix-platform/src/win32/util.rs — UTF-8 / UTF-16 conversion helpers
// ============================================================================

#![cfg(windows)]

use std::ptr;

// ════════════════════════════════════════════════════════════════════════════
// to_utf8 — 将 UTF-16 宽字符切片转换为 UTF-8 String
// ════════════════════════════════════════════════════════════════════════════

pub fn to_utf8(wstr: &[u16]) -> String {
    if wstr.is_empty() {
        return String::new();
    }

    // Find null terminator if present
    let len = wstr.iter().position(|&c| c == 0).unwrap_or(wstr.len());

    if len == 0 {
        return String::new();
    }

    unsafe {
        // First call: get required buffer size (including null)
        let needed = WideCharToMultiByte(
            CP_UTF8,
            0,
            wstr.as_ptr(),
            len as i32,
            ptr::null_mut(),
            0,
            ptr::null(),
            ptr::null_mut(),
        );

        if needed <= 0 {
            return String::new();
        }

        let buf_size = needed as usize;
        let mut buf = vec![0u8; buf_size];

        let written = WideCharToMultiByte(
            CP_UTF8,
            0,
            wstr.as_ptr(),
            len as i32,
            buf.as_mut_ptr(),
            needed,
            ptr::null(),
            ptr::null_mut(),
        );

        if written > 0 {
            buf.truncate(written as usize);
            String::from_utf8_unchecked(buf)
        } else {
            String::new()
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// to_wide — 将 UTF-8 字符串转换为以 null 结尾的 UTF-16 Vec<u16>
// ════════════════════════════════════════════════════════════════════════════

pub fn to_wide(utf8: &str) -> Vec<u16> {
    if utf8.is_empty() {
        return vec![0u16];
    }

    unsafe {
        // First call: get required buffer size
        let needed = MultiByteToWideChar(
            CP_UTF8,
            0,
            utf8.as_ptr(),
            utf8.len() as i32,
            ptr::null_mut(),
            0,
        );

        if needed <= 0 {
            return vec![0u16];
        }

        let buf_size = needed as usize;
        let mut buf = vec![0u16; buf_size];

        let written = MultiByteToWideChar(
            CP_UTF8,
            0,
            utf8.as_ptr(),
            utf8.len() as i32,
            buf.as_mut_ptr(),
            needed,
        );

        if written > 0 {
            buf.truncate(written as usize);
            buf.push(0); // null terminate
            buf
        } else {
            vec![0u16]
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// Raw FFI — kernel32.dll codepage conversion APIs
// ════════════════════════════════════════════════════════════════════════════

const CP_UTF8: u32 = 65001;

#[link(name = "kernel32")]
extern "system" {
    fn WideCharToMultiByte(
        CodePage: u32,
        dwFlags: u32,
        lpWideCharStr: *const u16,
        cchWideChar: i32,
        lpMultiByteStr: *mut u8,
        cbMultiByte: i32,
        lpDefaultChar: *const u8,
        lpUsedDefaultChar: *mut i32,
    ) -> i32;

    fn MultiByteToWideChar(
        CodePage: u32,
        dwFlags: u32,
        lpMultiByteStr: *const u8,
        cbMultiByte: i32,
        lpWideCharStr: *mut u16,
        cchWideChar: i32,
    ) -> i32;

    fn GetLastError() -> u32;
    fn FormatMessageW(
        dwFlags: u32,
        lpSource: *const std::ffi::c_void,
        dwMessageId: u32,
        dwLanguageId: u32,
        lpBuffer: *mut u16,
        nSize: u32,
        Arguments: *const std::ffi::c_void,
    ) -> u32;
}

const FORMAT_MESSAGE_FROM_SYSTEM: u32 = 0x00001000;
const FORMAT_MESSAGE_IGNORE_INSERTS: u32 = 0x00000200;

/// Get a human-readable error message from the last Win32 error.
pub fn get_last_error_string() -> String {
    unsafe {
        let code = GetLastError();
        if code == 0 {
            return "no error".to_string();
        }
        let mut buf = vec![0u16; 256];
        let len = FormatMessageW(
            FORMAT_MESSAGE_FROM_SYSTEM | FORMAT_MESSAGE_IGNORE_INSERTS,
            std::ptr::null(),
            code,
            0,
            buf.as_mut_ptr(),
            buf.len() as u32,
            std::ptr::null(),
        );
        if len > 0 {
            buf.truncate(len as usize);
            let msg = to_utf8(&buf);
            format!("Win32 error {}: {}", code, msg.trim())
        } else {
            format!("Win32 error {}", code)
        }
    }
}

/// Create a diagnostic error from the last Win32 error.
pub fn win32_diag(code: crate::diag::Errc, context: &str) -> crate::diag::Error {
    let msg = format!("{}: {}", context, get_last_error_string());
    crate::diag::Error::new(code, msg)
}
