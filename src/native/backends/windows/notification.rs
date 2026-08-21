// ============================================================================
// native/backends/windows/notification.rs — Windows notification (INotification)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]
#![allow(nonstandard_style)]

use crate::core::error::Errc;
use crate::core::error::{Error, Result};
use crate::platform::system::INotification;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationOwnerAction {
    Ignore,
    Add { owner: usize },
    Modify { owner: usize },
    Move { previous: usize, owner: usize },
}

pub(crate) fn notification_owner_action(
    active_owner: usize,
    target_owner: usize,
) -> NotificationOwnerAction {
    if target_owner == 0 {
        NotificationOwnerAction::Ignore
    } else if active_owner == 0 {
        NotificationOwnerAction::Add {
            owner: target_owner,
        }
    } else if active_owner == target_owner {
        NotificationOwnerAction::Modify {
            owner: target_owner,
        }
    } else {
        NotificationOwnerAction::Move {
            previous: active_owner,
            owner: target_owner,
        }
    }
}

// Windows 通知后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsNotification {
    hwnd: *mut std::ffi::c_void,
    active_owner: usize,
}

impl WindowsNotification {
    // 创建未绑定窗口的通知后端。
    pub(crate) fn new() -> Self {
        Self {
            hwnd: ptr::null_mut(),
            active_owner: 0,
        }
    }
    // 绑定当前平台窗口句柄。
    pub(crate) fn set_hwnd(&mut self, hwnd: *mut std::ffi::c_void) {
        self.hwnd = hwnd;
    }
    /// Remove the notification icon. Called on Drop.
    // 移除当前通知区域图标所有者。
    pub(crate) fn remove_icon(&mut self) {
        let owner = std::mem::take(&mut self.active_owner);
        if owner != 0 {
            Self::remove_icon_for(owner);
        }
    }

    fn remove_icon_for(owner: usize) {
        // SAFETY: 零初始化对 NOTIFYICONDATAW 的标量、句柄和数组字段均有效，cbSize 随后设置且 owner 仅作不透明 HWND 使用。
        unsafe {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = owner as *mut std::ffi::c_void;
            nid.uID = 1;
            Shell_NotifyIconW(NIM_DELETE, &mut nid);
        }
    }
}

impl Default for WindowsNotification {
    fn default() -> Self {
        Self::new()
    }
}

impl INotification for WindowsNotification {
    fn show(&mut self, title: &str, message: &str) -> Result<()> {
        let action = notification_owner_action(self.active_owner, self.hwnd as usize);
        let (owner, operation) = match action {
            NotificationOwnerAction::Ignore => return Ok(()),
            NotificationOwnerAction::Add { owner } => (owner, NIM_ADD),
            NotificationOwnerAction::Modify { owner } => (owner, NIM_MODIFY),
            NotificationOwnerAction::Move { previous, owner } => {
                Self::remove_icon_for(previous);
                self.active_owner = 0;
                (owner, NIM_ADD)
            }
        };
        let hwnd = owner as *mut std::ffi::c_void;
        if hwnd.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "WindowsNotification::show: owner HWND is null",
            ));
        }
        // SAFETY: NOTIFYICONDATAW 的尺寸、字符串边界和有效 HWND 均在调用前设置，Shell_NotifyIconW 不保留 Rust 借用。
        unsafe {
            let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
            nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
            nid.hWnd = hwnd;
            nid.uID = 1;
            nid.uFlags = NIF_INFO | NIF_ICON | NIF_TIP;
            nid.uCallbackMessage = WM_APP_NOTIFY;
            copy_notification_text(&mut nid.szInfoTitle, title);
            copy_notification_text(&mut nid.szInfo, message);
            nid.dwInfoFlags = NIIF_INFO;
            nid.hIcon = LoadIconW(ptr::null_mut(), IDI_APPLICATION as *const u16);
            if Shell_NotifyIconW(operation, &mut nid) != 0 {
                self.active_owner = owner;
                Ok(())
            } else if operation == NIM_MODIFY {
                self.active_owner = 0;
                if Shell_NotifyIconW(NIM_ADD, &mut nid) != 0 {
                    self.active_owner = owner;
                    Ok(())
                } else {
                    Err(Error::new(
                        Errc::PlatformError,
                        "WindowsNotification::show: Shell_NotifyIconW(NIM_ADD) fallback failed",
                    ))
                }
            } else {
                Err(Error::new(
                    Errc::PlatformError,
                    "WindowsNotification::show: Shell_NotifyIconW failed",
                ))
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
// SAFETY: Shell_NotifyIconW 声明对应 shell32 ABI，调用方提供尺寸正确且在同步调用期间可写的通知结构。
unsafe extern "system" {
    fn Shell_NotifyIconW(dwMessage: u32, lpdata: *mut NOTIFYICONDATAW) -> i32;
}

use super::ffi::LoadIconW;
