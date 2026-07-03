// ============================================================================
// bindings.rs — Windows 结构体定义（与 windows crate 重复，保留为过渡期使用）
//
// 这些定义与 windows crate 中的类型对应，但因该 crate 的 newtype 包装
// （HWND/*mut c_void, WPARAM(usize) 等）与本项目的 raw-pointer 风格不兼容，
// 暂时保留本地定义。后续逐步迁移到 windows crate。
// ============================================================================

#![cfg(windows)]
#![allow(nonstandard_style)]
#![allow(clippy::upper_case_acronyms)]
// Allow non-standard names like RECT, MSG, WNDCLASSEXW for Windows API compatibility.

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
pub(super) struct RECT {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
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
