// ============================================================================
// bindings.rs - local Win32 raw FFI structs used by the Windows backend.
//
// These definitions keep backend internals in raw-pointer form at the native
// boundary. Higher layers only see native traits and platform-neutral types.
// ============================================================================
#![cfg(windows)]
#![allow(nonstandard_style)]
#![allow(clippy::upper_case_acronyms)]
// Allow Win32 names such as RECT, MSG, and WNDCLASSEXW at the FFI boundary.

// ════════════════════════════════════════════════════════════════════════════
// 数据结构（仅用于窗口管理 + 事件循环）
// ════════════════════════════════════════════════════════════════════════════

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct POINT {
    pub x: i32,
    pub y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[repr(C)]
pub(crate) struct MONITORINFO {
    pub cbSize: u32,
    pub rcMonitor: RECT,
    pub rcWork: RECT,
    pub dwFlags: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub(super) struct WINDOWPLACEMENT {
    pub length: u32,
    pub flags: u32,
    pub showCmd: u32,
    pub ptMinPosition: POINT,
    pub ptMaxPosition: POINT,
    pub rcNormalPosition: RECT,
}

#[repr(C)]
pub(super) struct COMPOSITIONFORM {
    pub dwStyle: u32,
    pub ptCurrentPos: POINT,
    pub rcArea: RECT,
}

#[repr(C)]
pub(super) struct CANDIDATEFORM {
    pub dwIndex: u32,
    pub dwStyle: u32,
    pub ptCurrentPos: POINT,
    pub rcArea: RECT,
}

#[repr(C)]
pub(super) struct MSG {
    pub hwnd: *mut std::ffi::c_void,
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub time: u32,
    pub pt: POINT,
}

impl Default for MSG {
    fn default() -> Self {
        unsafe { std::mem::zeroed() }
    }
}

#[repr(C)]
pub(super) struct WNDCLASSEXW {
    pub cbSize: u32,
    pub style: u32,
    pub lpfnWndProc:
        Option<unsafe extern "system" fn(*mut std::ffi::c_void, u32, usize, isize) -> isize>,
    pub cbClsExtra: i32,
    pub cbWndExtra: i32,
    pub hInstance: *mut std::ffi::c_void,
    pub hIcon: *mut std::ffi::c_void,
    pub hCursor: *mut std::ffi::c_void,
    pub hbrBackground: *mut std::ffi::c_void,
    pub lpszMenuName: *const u16,
    pub lpszClassName: *const u16,
    pub hIconSm: *mut std::ffi::c_void,
}

#[repr(C)]
pub(super) struct CREATESTRUCTW {
    pub lpCreateParams: *mut std::ffi::c_void,
    pub hInstance: *mut std::ffi::c_void,
    pub hMenu: *mut std::ffi::c_void,
    pub hwndParent: *mut std::ffi::c_void,
    pub cy: i32,
    pub cx: i32,
    pub y: i32,
    pub x: i32,
    pub style: u32,
    pub lpszName: *const u16,
    pub lpszClass: *const u16,
    pub dwExStyle: u32,
}
