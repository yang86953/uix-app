// ============================================================================
// uix-platform/src/win32/keyboard.rs — Win32 keyboard input (IKeyboard)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::platform::types::KeyCode;
use crate::platform::IKeyboard;

pub struct Win32Keyboard;

impl Win32Keyboard {
    pub fn new() -> Self {
        Self
    }
}

impl Default for Win32Keyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl IKeyboard for Win32Keyboard {
    fn is_down(&self, key: KeyCode) -> bool {
        let disc = key as u32;
        let vk = if disc == KeyCode::Shift as u32 {
            VK_SHIFT
        } else if disc == KeyCode::Ctrl as u32 {
            VK_CONTROL
        } else if disc == KeyCode::Alt as u32 {
            VK_MENU
        } else if disc == KeyCode::Super as u32 {
            VK_LWIN
        } else if (KeyCode::A as u32..=KeyCode::Z as u32).contains(&disc) {
            0x41 + (disc - KeyCode::A as u32)
        } else if (KeyCode::Num0 as u32..=KeyCode::Num9 as u32).contains(&disc) {
            0x30 + (disc - KeyCode::Num0 as u32)
        } else if (KeyCode::F1 as u32..=KeyCode::F12 as u32).contains(&disc) {
            0x70 + (disc - KeyCode::F1 as u32)
        } else if disc == KeyCode::Left as u32 {
            VK_LEFT
        } else if disc == KeyCode::Right as u32 {
            VK_RIGHT
        } else if disc == KeyCode::Up as u32 {
            VK_UP
        } else if disc == KeyCode::Down as u32 {
            VK_DOWN
        } else if disc == KeyCode::Home as u32 {
            VK_HOME
        } else if disc == KeyCode::End as u32 {
            VK_END
        } else if disc == KeyCode::PageUp as u32 {
            VK_PRIOR
        } else if disc == KeyCode::PageDown as u32 {
            VK_NEXT
        } else if disc == KeyCode::Enter as u32 {
            VK_RETURN
        } else if disc == KeyCode::Escape as u32 {
            VK_ESCAPE
        } else if disc == KeyCode::Backspace as u32 {
            VK_BACK
        } else if disc == KeyCode::Delete as u32 {
            VK_DELETE
        } else if disc == KeyCode::Tab as u32 {
            VK_TAB
        } else if disc == KeyCode::Space as u32 {
            VK_SPACE
        } else if disc == KeyCode::Insert as u32 {
            VK_INSERT
        } else {
            return false;
        };
        is_key_down(vk)
    }

    fn idle_ms(&self) -> u32 {
        unsafe {
            let mut info = LASTINPUTINFO {
                cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
                dwTime: 0,
            };
            if GetLastInputInfo(&mut info) != 0 {
                let tick = GetTickCount();
                tick.saturating_sub(info.dwTime)
            } else {
                0
            }
        }
    }

    fn double_click_ms(&self) -> u32 {
        unsafe { GetDoubleClickTime() }
    }
}

// ── FFI ──

#[repr(C)]
struct LASTINPUTINFO {
    cbSize: u32,
    dwTime: u32,
}

const VK_SHIFT: u32 = 0x10;
const VK_CONTROL: u32 = 0x11;
const VK_MENU: u32 = 0x12;
const VK_LWIN: u32 = 0x5B;
const VK_LEFT: u32 = 0x25;
const VK_UP: u32 = 0x26;
const VK_RIGHT: u32 = 0x27;
const VK_DOWN: u32 = 0x28;
const VK_RETURN: u32 = 0x0D;
const VK_ESCAPE: u32 = 0x1B;
const VK_BACK: u32 = 0x08;
const VK_DELETE: u32 = 0x2E;
const VK_TAB: u32 = 0x09;
const VK_SPACE: u32 = 0x20;
const VK_INSERT: u32 = 0x2D;
const VK_HOME: u32 = 0x24;
const VK_END: u32 = 0x23;
const VK_PRIOR: u32 = 0x21;
const VK_NEXT: u32 = 0x22;

fn is_key_down(vk: u32) -> bool {
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

#[link(name = "user32")]
extern "system" {
    fn GetAsyncKeyState(vKey: i32) -> i16;
    fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
    fn GetTickCount() -> u32;
    fn GetDoubleClickTime() -> u32;
}
