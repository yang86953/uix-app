// ============================================================================
// ffi.rs — Windows FFI 函数声明（与 windows crate 重复，保留为过渡期使用）
//
// 这些 FFI 声明与 windows crate 中的函数对应，但因 crate 的签名使用
// HWND/HINSTANCE 等包装类型而非 raw pointer，与本项目的风格不兼容，
// 暂时保留本地声明。后续逐步迁移到 windows crate。
// ============================================================================

#![cfg(windows)]
#![allow(nonstandard_style)]

use super::bindings::{FLASHWINFO, MSG, POINT, RECT, WNDCLASSEXW};

// ════════════════════════════════════════════════════════════════════════════
// kernel32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "kernel32")]
extern "system" {
    pub(super) fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
}

// ════════════════════════════════════════════════════════════════════════════
// user32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "user32")]
extern "system" {
    pub(super) fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;

    pub(super) fn CreateWindowExW(
        dwExStyle: u32,
        lpClassName: *const u16,
        lpWindowName: *const u16,
        dwStyle: u32,
        x: i32,
        y: i32,
        nWidth: i32,
        nHeight: i32,
        hWndParent: *mut std::ffi::c_void,
        hMenu: *mut std::ffi::c_void,
        hInstance: *mut std::ffi::c_void,
        lpParam: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;

    pub(super) fn DestroyWindow(hwnd: *mut std::ffi::c_void) -> i32;

    pub(super) fn AdjustWindowRectEx(
        lpRect: *mut RECT,
        dwStyle: u32,
        bMenu: i32,
        dwExStyle: u32,
    ) -> i32;

    pub(super) fn DefWindowProcW(
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;

    pub(super) fn GetMessageW(
        lpMsg: *mut MSG,
        hwnd: *mut std::ffi::c_void,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
    ) -> i32;

    pub(super) fn PeekMessageW(
        lpMsg: *mut MSG,
        hwnd: *mut std::ffi::c_void,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
        wRemoveMsg: u32,
    ) -> i32;

    pub(super) fn TranslateMessage(lpMsg: *const MSG) -> i32;

    pub(super) fn DispatchMessageW(lpMsg: *const MSG) -> isize;

    pub(super) fn PostQuitMessage(nExitCode: i32);

    pub(super) fn ShowWindow(hwnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;

    pub(super) fn SetWindowTextW(hwnd: *mut std::ffi::c_void, lpString: *const u16) -> i32;

    pub(super) fn SetWindowPos(
        hwnd: *mut std::ffi::c_void,
        hWndInsertAfter: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;

    pub(super) fn SetWindowLongW(hwnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: i32) -> i32;

    pub(super) fn GetWindowLongW(hwnd: *mut std::ffi::c_void, nIndex: i32) -> i32;

    pub(super) fn SetWindowLongPtrW(
        hwnd: *mut std::ffi::c_void,
        nIndex: i32,
        dwNewLong: isize,
    ) -> isize;

    pub(super) fn GetWindowLongPtrW(hwnd: *mut std::ffi::c_void, nIndex: i32) -> isize;

    pub(super) fn SetLayeredWindowAttributes(
        hwnd: *mut std::ffi::c_void,
        crKey: u32,
        bAlpha: u8,
        dwFlags: u32,
    ) -> i32;

    pub(super) fn FlashWindowEx(pfwi: *mut FLASHWINFO) -> i32;

    pub(super) fn GetSystemMetrics(nIndex: i32) -> i32;

    pub(super) fn LoadIconW(
        hInstance: *mut std::ffi::c_void,
        lpIconName: *const u16,
    ) -> *mut std::ffi::c_void;

    pub(super) fn LoadImageW(
        hInst: *mut std::ffi::c_void,
        name: *const u16,
        typ: u32,
        cx: i32,
        cy: i32,
        fuLoad: u32,
    ) -> *mut std::ffi::c_void;

    pub(super) fn LoadCursorW(
        hInstance: *mut std::ffi::c_void,
        lpCursorName: *const u16,
    ) -> *mut std::ffi::c_void;

    pub(super) fn SetCursor(hCursor: *mut std::ffi::c_void) -> *mut std::ffi::c_void;

    pub(super) fn SetCapture(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;

    pub(super) fn ReleaseCapture() -> i32;

    pub(super) fn ScreenToClient(hwnd: *mut std::ffi::c_void, lpPoint: *mut POINT) -> i32;

    pub(super) fn SendMessageW(
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;

    pub(super) fn GetAsyncKeyState(vKey: i32) -> i16;

    pub(super) fn DragAcceptFiles(hwnd: *mut std::ffi::c_void, fAccept: i32);

    pub(super) fn DragQueryFileW(
        hdrop: *mut std::ffi::c_void,
        iFile: u32,
        lpszFile: *mut u16,
        cch: u32,
    ) -> u32;

    pub(super) fn DragQueryPoint(hdrop: *mut std::ffi::c_void, lppt: *mut POINT) -> i32;

    pub(super) fn DragFinish(hdrop: *mut std::ffi::c_void);

    pub(super) fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// gdi32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "gdi32")]
extern "system" {
    pub(super) fn GetDeviceCaps(hdc: *mut std::ffi::c_void, nIndex: i32) -> i32;
}
