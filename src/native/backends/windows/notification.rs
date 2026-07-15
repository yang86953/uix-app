// ============================================================================
// native/backends/windows/notification.rs — Windows notification (INotification)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::native::traits::system::INotification;
use std::ptr;

pub(crate) fn copy_notification_text(destination: &mut [u16], text: &str) {
    destination.fill(0);
    let Some(capacity) = destination.len().checked_sub(1) else {
        return;
    };
    let mut written = 0;
    for character in text.chars().take_while(|character| *character != '\0') {
        let mut encoded = [0_u16; 2];
        let units = character.encode_utf16(&mut encoded);
        if written + units.len() > capacity {
            break;
        }
        destination[written..written + units.len()].copy_from_slice(units);
        written += units.len();
    }
}

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
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = self.hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_INFO | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_APP_NOTIFY;
            copy_notification_text(&mut nid.szInfoTitle, title);
            copy_notification_text(&mut nid.szInfo, message);
            nid.dwInfoFlags = NIIF_INFO;
            nid.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION as *const u16);
            let operation = if self.active { NIM_MODIFY } else { NIM_ADD };
            if Shell_NotifyIconW(operation, &mut nid) != 0 {
                self.active = true;
            } else if self.active {
                self.active = false;
                self.active = Shell_NotifyIconW(NIM_ADD, &mut nid) != 0;
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

use super::ffi::LoadIconW;
