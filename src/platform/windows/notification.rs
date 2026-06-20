// ============================================================================
// uix-platform/src/windows/notification.rs — Windows notification (INotification)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::platform::windows::util::to_wide;
use crate::platform::INotification;
use std::ptr;

pub struct WindowsNotification {
    hwnd: *mut std::ffi::c_void,
    active: bool,
}

impl WindowsNotification {
    pub fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
            active: false,
        }
    }
    pub fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
    pub fn is_active(&self) -> bool {
        self.active
    }
    /// Remove the notification icon. Called on Drop.
    pub fn remove_icon(&mut self) {
        if self.active && !self.hwnd.is_null() {
            unsafe {
                let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
                nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
                nid.hWnd = self.hwnd;
                nid.uID = 1;
                Shell_NotifyIconW(NIM_DELETE, &mut nid);
            }
            self.active = false;
        }
    }
}

impl Default for WindowsNotification {
    fn default() -> Self {
        Self::new()
    }
}

impl INotification for WindowsNotification {
    fn show(&mut self, title: &str, message: &str) {
        if self.hwnd.is_null() {
            return;
        }
        unsafe {
            let wide_title = to_wide(title);
            let wide_msg = to_wide(message);
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_INFO | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_APP_NOTIFY;
            let title_len = wide_title.len().min(64);
            nid.szInfoTitle[..title_len].copy_from_slice(&wide_title[..title_len]);
            if title_len < 64 {
                nid.szInfoTitle[title_len] = 0;
            }
            let msg_len = wide_msg.len().min(256);
            nid.szInfo[..msg_len].copy_from_slice(&wide_msg[..msg_len]);
            if msg_len < 256 {
                nid.szInfo[msg_len] = 0;
            }
            nid.dwInfoFlags = NIIF_INFO;
            nid.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION as *const u16);
            if self.active {
                Shell_NotifyIconW(NIM_MODIFY, &mut nid);
            } else {
                Shell_NotifyIconW(NIM_ADD, &mut nid);
                self.active = true;
            }
        }
    }
}

impl Drop for WindowsNotification {
    fn drop(&mut self) {
        self.remove_icon();
    }
}

// ── FFI ──

#[repr(C)]
struct NOTIFYICONDATAW {
    cbSize: u32,
    hWnd: *mut std::ffi::c_void,
    uID: u32,
    uFlags: u32,
    uCallbackMessage: u32,
    hIcon: *mut std::ffi::c_void,
    szTip: [u16; 128],
    dwState: u32,
    dwStateMask: u32,
    szInfo: [u16; 256],
    uVersion: u32,
    szInfoTitle: [u16; 64],
    dwInfoFlags: u32,
    guidItem: [u8; 16],
    hBalloonIcon: *mut std::ffi::c_void,
}

const NIF_ICON: u32 = 0x0002;
const NIF_INFO: u32 = 0x0010;
const NIF_TIP: u32 = 0x0004;
const NIIF_INFO: u32 = 0x0001;
const NIM_ADD: u32 = 0;
const NIM_MODIFY: u32 = 1;
const NIM_DELETE: u32 = 2;
const WM_APP_NOTIFY: u32 = 0x8000;
const IDI_APPLICATION: u16 = 32512;

#[link(name = "shell32")]
extern "system" {
    fn Shell_NotifyIconW(dwMessage: u32, lpdata: *mut NOTIFYICONDATAW) -> i32;
}
#[link(name = "user32")]
extern "system" {
    fn LoadIconW(hInstance: *mut std::ffi::c_void, lpIconName: *const u16)
        -> *mut std::ffi::c_void;
}
