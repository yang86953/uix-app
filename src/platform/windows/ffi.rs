// ============================================================================
// ffi.rs — Windows FFI 函数声明（集中管理）
//
// 设计原则：所有 extern "system" 声明集中在此文件。
// 子系统文件（clipboard.rs、cursor.rs 等）不再声明自己的 extern 块，
// 改为从本文件导入（use super::ffi::*）。
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

    // ── 控制台 ──
    pub(super) fn GetStdHandle(nStdHandle: u32) -> *mut std::ffi::c_void;
    pub(super) fn SetConsoleTextAttribute(hConsoleOutput: *mut std::ffi::c_void, wAttributes: u16) -> i32;
    pub(super) fn WriteConsoleA(
        hConsoleOutput: *mut std::ffi::c_void,
        lpBuffer: *const u8,
        nNumberOfCharsToWrite: u32,
        lpNumberOfCharsWritten: *mut u32,
        lpReserved: *mut std::ffi::c_void,
    ) -> i32;
    pub(super) fn GetConsoleScreenBufferInfo(
        hConsoleOutput: *mut std::ffi::c_void,
        lpConsoleScreenBufferInfo: *mut std::ffi::c_void,
    ) -> i32;
    pub(super) fn SetConsoleCursorInfo(
        hConsoleOutput: *mut std::ffi::c_void,
        lpConsoleCursorInfo: *const std::ffi::c_void,
    ) -> i32;
    pub(super) fn SetConsoleTitleW(lpConsoleTitle: *const u16) -> i32;

    // ── 系统信息 ──
    pub(super) fn RtlGetVersion(lpVersionInformation: *mut std::ffi::c_void) -> i32;
    pub(super) fn GetNativeSystemInfo(lpSystemInfo: *mut std::ffi::c_void);
    pub(super) fn GlobalMemoryStatusEx(lpBuffer: *mut std::ffi::c_void) -> i32;
    pub(super) fn GetComputerNameW(lpBuffer: *mut u16, nSize: *mut u32) -> i32;
    pub(super) fn GetTickCount64() -> u64;
    pub(super) fn GetUserNameW(lpBuffer: *mut u16, nSize: *mut u32) -> i32;
    pub(super) fn GetCurrentProcess() -> *mut std::ffi::c_void;
    pub(super) fn GetTickCount() -> u32;
    pub(super) fn GlobalAlloc(uFlags: u32, dwBytes: usize) -> *mut std::ffi::c_void;
    pub(super) fn GlobalLock(hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn GlobalUnlock(hMem: *mut std::ffi::c_void) -> i32;

    // ── 文件/路径 ──
    pub(super) fn GetTempPathW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
    pub(super) fn GetModuleFileNameW(hModule: *mut std::ffi::c_void, lpFilename: *mut u16, nSize: u32) -> u32;
    pub(super) fn GetCurrentDirectoryW(nBufferLength: u32, lpBuffer: *mut u16) -> u32;
    pub(super) fn GetFileAttributesW(lpFileName: *const u16) -> u32;
    pub(super) fn GetWindowsDirectoryW(lpBuffer: *mut u16, uSize: u32) -> u32;

    // ── 字符串/错误 ──
    pub(super) fn WideCharToMultiByte(
        CodePage: u32,
        dwFlags: u32,
        lpWideCharStr: *const u16,
        cchWideChar: u32,
        lpMultiByteStr: *mut u8,
        cbMultiByte: u32,
        lpDefaultChar: *const u8,
        lpUsedDefaultChar: *mut i32,
    ) -> i32;
    pub(super) fn MultiByteToWideChar(
        CodePage: u32,
        dwFlags: u32,
        lpMultiByteStr: *const u8,
        cbMultiByte: u32,
        lpWideCharStr: *mut u16,
        cchWideChar: u32,
    ) -> i32;
    pub(super) fn GetLastError() -> u32;
    pub(super) fn FormatMessageW(
        dwFlags: u32,
        lpSource: *const std::ffi::c_void,
        dwMessageId: u32,
        dwLanguageId: u32,
        lpBuffer: *mut u16,
        nSize: u32,
        Arguments: *mut u8,
    ) -> u32;
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
    pub(super) fn AdjustWindowRectEx(lpRect: *mut RECT, dwStyle: u32, bMenu: i32, dwExStyle: u32) -> i32;
    pub(super) fn DefWindowProcW(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub(super) fn GetMessageW(lpMsg: *mut MSG, hwnd: *mut std::ffi::c_void, wMsgFilterMin: u32, wMsgFilterMax: u32) -> i32;
    pub(super) fn PeekMessageW(lpMsg: *mut MSG, hwnd: *mut std::ffi::c_void, wMsgFilterMin: u32, wMsgFilterMax: u32, wRemoveMsg: u32) -> i32;
    pub(super) fn TranslateMessage(lpMsg: *const MSG) -> i32;
    pub(super) fn DispatchMessageW(lpMsg: *const MSG) -> isize;
    pub(super) fn PostQuitMessage(nExitCode: i32);
    pub(super) fn ShowWindow(hwnd: *mut std::ffi::c_void, nCmdShow: i32) -> i32;
    pub(super) fn SetWindowTextW(hwnd: *mut std::ffi::c_void, lpString: *const u16) -> i32;
    pub(super) fn SetWindowPos(hwnd: *mut std::ffi::c_void, hWndInsertAfter: *mut std::ffi::c_void, x: i32, y: i32, cx: i32, cy: i32, uFlags: u32) -> i32;
    pub(super) fn SetWindowLongW(hwnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: i32) -> i32;
    pub(super) fn GetWindowLongW(hwnd: *mut std::ffi::c_void, nIndex: i32) -> i32;
    pub(super) fn SetWindowLongPtrW(hwnd: *mut std::ffi::c_void, nIndex: i32, dwNewLong: isize) -> isize;
    pub(super) fn GetWindowLongPtrW(hwnd: *mut std::ffi::c_void, nIndex: i32) -> isize;
    pub(super) fn SetLayeredWindowAttributes(hwnd: *mut std::ffi::c_void, crKey: u32, bAlpha: u8, dwFlags: u32) -> i32;
    pub(super) fn FlashWindowEx(pfwi: *mut FLASHWINFO) -> i32;
    pub(super) fn GetSystemMetrics(nIndex: i32) -> i32;
    pub(super) fn LoadIconW(hInstance: *mut std::ffi::c_void, lpIconName: *const u16) -> *mut std::ffi::c_void;
    pub(super) fn LoadImageW(hInst: *mut std::ffi::c_void, name: *const u16, typ: u32, cx: i32, cy: i32, fuLoad: u32) -> *mut std::ffi::c_void;
    pub(super) fn LoadCursorW(hInstance: *mut std::ffi::c_void, lpCursorName: *const u16) -> *mut std::ffi::c_void;
    pub(super) fn SetCursor(hCursor: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn SetCapture(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn ReleaseCapture() -> i32;
    pub(super) fn ScreenToClient(hwnd: *mut std::ffi::c_void, lpPoint: *mut POINT) -> i32;
    pub(super) fn SendMessageW(hwnd: *mut std::ffi::c_void, msg: u32, wparam: usize, lparam: isize) -> isize;
    pub(super) fn GetAsyncKeyState(vKey: i32) -> i16;
    pub(super) fn DragAcceptFiles(hwnd: *mut std::ffi::c_void, fAccept: i32);
    pub(super) fn DragQueryFileW(hdrop: *mut std::ffi::c_void, iFile: u32, lpszFile: *mut u16, cch: u32) -> u32;
    pub(super) fn DragQueryPoint(hdrop: *mut std::ffi::c_void, lppt: *mut POINT) -> i32;
    pub(super) fn DragFinish(hdrop: *mut std::ffi::c_void);
    pub(super) fn GetDC(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn ReleaseDC(hwnd: *mut std::ffi::c_void, hdc: *mut std::ffi::c_void) -> i32;
    pub(super) fn SetTimer(hwnd: *mut std::ffi::c_void, nIDEvent: u32, uElapse: u32, lpTimerFunc: Option<unsafe extern "system" fn()>) -> usize;
    pub(super) fn KillTimer(hwnd: *mut std::ffi::c_void, uIDEvent: u32) -> i32;

    // ── 光标 ──
    pub(super) fn ShowCursor(bShow: i32) -> i32;
    pub(super) fn GetCursorPos(lpPoint: *mut POINT) -> i32;
    pub(super) fn SetCursorPos(x: i32, y: i32) -> i32;
    pub(super) fn ClipCursor(lpRect: *const RECT) -> i32;
    pub(super) fn GetWindowRect(hwnd: *mut std::ffi::c_void, lpRect: *mut RECT) -> i32;

    // ── 剪贴板 ──
    pub(super) fn OpenClipboard(hwnd: *mut std::ffi::c_void) -> i32;
    pub(super) fn CloseClipboard() -> i32;
    pub(super) fn GetClipboardData(uFormat: u32) -> *mut std::ffi::c_void;
    pub(super) fn SetClipboardData(uFormat: u32, hMem: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn EmptyClipboard() -> i32;
    pub(super) fn IsClipboardFormatAvailable(uFormat: u32) -> i32;
    pub(super) fn GetPriorityClipboardFormat(paFormatPriorityList: *const u32, cFormats: i32) -> i32;

    // ── 键盘 ──
    pub(super) fn GetLastInputInfo(plii: *mut std::ffi::c_void) -> i32;
    pub(super) fn GetDoubleClickTime() -> u32;
}

// ════════════════════════════════════════════════════════════════════════════
// gdi32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "gdi32")]
extern "system" {
    pub(super) fn GetDeviceCaps(hdc: *mut std::ffi::c_void, nIndex: i32) -> i32;
    pub(super) fn CreateDIBSection(
        hdc: *mut std::ffi::c_void,
        pbmi: *const std::ffi::c_void,
        usage: u32,
        ppvBits: *mut *mut std::ffi::c_void,
        hSection: *mut std::ffi::c_void,
        offset: u32,
    ) -> *mut std::ffi::c_void;
    pub(super) fn SelectObject(hdc: *mut std::ffi::c_void, h: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn DeleteObject(h: *mut std::ffi::c_void) -> i32;
    pub(super) fn DeleteDC(hdc: *mut std::ffi::c_void) -> i32;
    pub(super) fn BitBlt(
        hdc: *mut std::ffi::c_void,
        x: i32, y: i32, cx: i32, cy: i32,
        hdcSrc: *mut std::ffi::c_void,
        x1: i32, y1: i32,
        rop: u32,
    ) -> i32;
    pub(super) fn CreateCompatibleDC(hdc: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn GetDIBits(
        hdc: *mut std::ffi::c_void,
        hbm: *mut std::ffi::c_void,
        start: u32,
        cLines: u32,
        lpvBits: *mut std::ffi::c_void,
        lpbmi: *mut std::ffi::c_void,
        usage: u32,
    ) -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// shell32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "shell32")]
extern "system" {
    pub(super) fn Shell_NotifyIconW(dwMessage: u32, lpdata: *mut std::ffi::c_void) -> i32;
    pub(super) fn SHGetKnownFolderPath(
        rfid: *const std::ffi::c_void,
        dwFlags: u32,
        hToken: *mut std::ffi::c_void,
        ppszPath: *mut *mut u16,
    ) -> i32;
    pub(super) fn SHBrowseForFolderW(lpbi: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn SHGetPathFromIDListW(pidl: *mut std::ffi::c_void, pszPath: *mut u16) -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// comdlg32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "comdlg32")]
extern "system" {
    pub(super) fn GetOpenFileNameW(lpofn: *mut std::ffi::c_void) -> i32;
    pub(super) fn GetSaveFileNameW(lpofn: *mut std::ffi::c_void) -> i32;
}

// ════════════════════════════════════════════════════════════════════════════
// ole32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "ole32")]
extern "system" {
    pub(super) fn CoTaskMemFree(pv: *mut std::ffi::c_void);
}

// ════════════════════════════════════════════════════════════════════════════
// imm32
// ════════════════════════════════════════════════════════════════════════════

#[link(name = "imm32")]
extern "system" {
    pub(super) fn ImmAssociateContextEx(
        hwnd: *mut std::ffi::c_void,
        himc: *mut std::ffi::c_void,
        dwFlags: u32,
    ) -> i32;
    pub(super) fn ImmGetContext(hwnd: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
    pub(super) fn ImmReleaseContext(hwnd: *mut std::ffi::c_void, himc: *mut std::ffi::c_void) -> i32;
    pub(super) fn ImmSetCompositionWindow(
        himc: *mut std::ffi::c_void,
        lpCompForm: *mut std::ffi::c_void,
    ) -> i32;
}
