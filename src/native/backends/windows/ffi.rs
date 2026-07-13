// ============================================================================
// ffi.rs — Windows FFI 函数声明（集中管理）
//
// 设计原则：所有 extern "system" 声明集中在此文件。
// 子系统文件（clipboard.rs、cursor.rs 等）不再声明自己的 extern 块，
// 改为从本文件导入（use super::ffi::*）。
// ============================================================================

#![cfg(windows)]
#![allow(nonstandard_style)]

use super::bindings::{CANDIDATEFORM, COMPOSITIONFORM, MSG, POINT, RECT, WNDCLASSEXW};

// ════════════════════════════════════════════════════════════════════════════
// kernel32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "kernel32")]
extern "system" {
    pub(crate) fn GetModuleHandleW(lpModuleName: *const u16) -> *mut std::ffi::c_void;
    pub(crate) fn GetLastError() -> u32;

    pub(crate) fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
    pub(crate) fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(crate) fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;

}

// ════════════════════════════════════════════════════════════════════════════
// user32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "user32")]
extern "system" {
    pub(crate) fn RegisterClassExW(lpwcx: *const WNDCLASSEXW) -> u16;
    pub(crate) fn CreateWindowExW(
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
    pub(crate) fn DestroyWindow(hwnd: *mut std::ffi::c_void) -> i32;
    pub(crate) fn IsWindow(hwnd: *mut std::ffi::c_void) -> i32;
    pub(crate) fn AdjustWindowRectEx(
        lpRect: *mut RECT,
        dwStyle: u32,
        bMenu: i32,
        dwExStyle: u32,
    ) -> i32;
    pub(crate) fn DefWindowProcW(
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize;
    pub(crate) fn PeekMessageW(
        lpMsg: *mut MSG,
        hwnd: *mut std::ffi::c_void,
        wMsgFilterMin: u32,
        wMsgFilterMax: u32,
        wRemoveMsg: u32,
    ) -> i32;
    pub(crate) fn PostMessageW(
        hwnd: *mut std::ffi::c_void,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> i32;
    /// 等待消息或超时。返回 WAIT_TIMEOUT 表示超时，WAIT_FAILED 表示失败。
    pub(crate) fn MsgWaitForMultipleObjects(
        nCount: u32,
        pHandles: *const std::ffi::c_void,
        fWaitAll: i32,
        dwMilliseconds: u32,
        dwWakeMask: u32,
    ) -> u32;
    pub(crate) fn TranslateMessage(lpMsg: *const MSG) -> i32;
    pub(crate) fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    pub(crate) fn ShowWindow(hwnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    pub(crate) fn SetWindowTextW(hwnd: *mut std::ffi::c_void, lpString: *const u16) -> i32;
    pub(crate) fn SetWindowPos(
        hwnd: *mut std::ffi::c_void,
        hWndInsertAfter: *mut std::ffi::c_void,
        x: i32,
        y: i32,
        cx: i32,
        cy: i32,
        uFlags: u32,
    ) -> i32;
    pub(crate) fn SetWindowLongW(hwnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: i32) -> i32;
    pub(crate) fn GetWindowLongW(hwnd: *mut std::ffi::c_void, nIndex: i32) -> i32;
    pub(crate) fn SetWindowLongPtrW(
        hwnd: *mut std::ffi::c_void,
        nIndex: i32,
        dwNewLong: isize,
    ) -> isize;
    pub(crate) fn GetWindowLongPtrW(hwnd: *mut std::ffi::c_void, nIndex: i32) -> isize;
    pub(crate) fn SetLayeredWindowAttributes(
        hwnd: *mut std::ffi::c_void,
        crKey: u32,
        bAlpha: u8,
        dwFlags: u32,
    ) -> i32;
    pub(crate) fn GetSystemMetrics(nIndex: i32) -> i32;
    pub(crate) fn LoadIconW(
        hInstance: *mut std::ffi::c_void,
        lpIconName: *const u16,
    ) -> *mut std::ffi::c_void;
    pub(crate) fn LoadCursorW(
        hInstance: *mut std::ffi::c_void,
        lpCursorName: *const u16,
    ) -> *mut std::ffi::c_void;
    pub(crate) fn SetCursor(hCursor: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(crate) fn SetCapture(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(crate) fn ReleaseCapture() -> i32;
    pub(crate) fn ScreenToClient(hwnd: *mut std::ffi::c_void, lpPoint: *mut POINT) -> i32;
    pub(crate) fn GetAsyncKeyState(vKey: i32) -> i16;
    pub(crate) fn DragAcceptFiles(hwnd: *mut std::ffi::c_void, fAccept: i32);
    pub(crate) fn DragQueryFileW(
        hdrop: *mut std::ffi::c_void,
        iFile: u32,
        lpszFile: *mut u16,
        cch: u32,
    ) -> u32;
    pub(crate) fn DragQueryPoint(hdrop: *mut std::ffi::c_void, lppt: *mut POINT) -> i32;
    pub(crate) fn DragFinish(hdrop: *mut std::ffi::c_void);
    pub(crate) fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(crate) fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    pub(crate) fn SetTimer(
        hwnd: *mut std::ffi::c_void,
        nIDEvent: u32,
        uElapse: u32,
        lpTimerFunc: Option<unsafe extern "system" fn()>,
    ) -> usize;
    pub(crate) fn KillTimer(hwnd: *mut std::ffi::c_void, uIDEvent: u32) -> i32;

    pub(crate) fn GetWindowRect(hwnd: *mut std::ffi::c_void, lpRect: *mut RECT) -> i32;
    pub(crate) fn GetClientRect(hwnd: *mut std::ffi::c_void, lpRect: *mut RECT) -> i32;

    // ── 剪贴板 ──
    pub(crate) fn OpenClipboard(hwnd: *mut std::ffi::c_void) -> i32;
    pub(crate) fn CloseClipboard() -> i32;
    pub(crate) fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
    pub(crate) fn SetClipboardData(
        uFormat: u32,
        hMem: *mut std::ffi::c_void,
    ) -> *mut std::ffi::c_void;
    pub(crate) fn EmptyClipboard() -> i32;
    pub(crate) fn GetPriorityClipboardFormat(
        paFormatPriorityList: *const u32,
        cFormats: i32,
    ) -> i32;

    // ── 键盘 ──
    // GetLastInputInfo 已在 keyboard.rs 中声明（使用具体类型）
}

// ════════════════════════════════════════════════════════════════════════════
// gdi32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "gdi32")]
extern "system" {
    pub(crate) fn GetDeviceCaps(hdc: *mut std::ffi::c_void, nIndex: i32) -> i32;
    // CreateDIBSection 已在 gdi_presenter.rs 中声明（使用具体 BITMAPINFO 类型）
}

// ════════════════════════════════════════════════════════════════════════════
// shell32（函数在各子系统文件中声明，避免参数类型冲突）
// comdlg32（函数在 file_dialog.rs 中声明）
// ole32（函数在 file_dialog.rs 和 filesystem.rs 中声明）
// ════════════════════════════════════════════════════════════════════════════

// ════════════════════════════════════════════════════════════════════════════
// advapi32 — 注册表
// ════════════════════════════════════════════════════════════════════════════

/// HKEY_CURRENT_USER 伪句柄（0x80000001 零扩展到指针宽度）
pub(crate) fn hkey_current_user() -> *mut std::ffi::c_void {
    0x8000_0001usize as *mut std::ffi::c_void
}

pub const KEY_READ: u32 = 0x20019;
pub const REG_DWORD: u32 = 4;
pub const ERROR_SUCCESS: i32 = 0;

#[link(name = "advapi32")]
extern "system" {
    pub(crate) fn RegOpenKeyExW(
        hKey: *mut std::ffi::c_void,
        lpSubKey: *const u16,
        ulOptions: u32,
        samDesired: u32,
        phkResult: *mut *mut std::ffi::c_void,
    ) -> i32;
    pub(crate) fn RegQueryValueExW(
        hKey: *mut std::ffi::c_void,
        lpValueName: *const u16,
        lpReserved: *mut u32,
        lpType: *mut u32,
        lpData: *mut u8,
        lpcbData: *mut u32,
    ) -> i32;
    pub(crate) fn RegCloseKey(hKey: *mut std::ffi::c_void) -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// imm32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "imm32")]
extern "system" {
    pub(crate) fn ImmAssociateContextEx(
        hwnd: *mut std::ffi::c_void,
        himc: *mut std::ffi::c_void,
        dwFlags: u32,
    ) -> i32;
    pub(crate) fn ImmGetContext(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(crate) fn ImmReleaseContext(
        hwnd: *mut std::ffi::c_void,
        himc: *mut std::ffi::c_void,
    ) -> i32;
    pub(crate) fn ImmGetCompositionStringW(
        himc: *mut std::ffi::c_void,
        index: u32,
        buffer: *mut std::ffi::c_void,
        buffer_len: u32,
    ) -> i32;
    pub(crate) fn ImmSetCompositionWindow(
        himc: *mut std::ffi::c_void,
        form: *const COMPOSITIONFORM,
    ) -> i32;
    pub(crate) fn ImmSetCandidateWindow(
        himc: *mut std::ffi::c_void,
        form: *const CANDIDATEFORM,
    ) -> i32;
}
