// ============================================================================
// uix-platform/src/windows/keyboard.rs — Windows keyboard input (IKeyboard)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::base::KeyCode;
use crate::platform::IKeyboard;

pub struct WindowsKeyboard;

impl WindowsKeyboard {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsKeyboard {
    fn default() -> Self {
        Self::new()
    }
}

impl IKeyboard for WindowsKeyboard {
    fn is_down(&self, key: KeyCode) -> bool {
        let disc = key as u32;
        let vk = match disc {
            // 修饰键
            d if d == KeyCode::Shift as u32 => VK_SHIFT,
            d if d == KeyCode::Ctrl as u32 => VK_CONTROL,
            d if d == KeyCode::Alt as u32 => VK_MENU,
            d if d == KeyCode::Super as u32 => VK_LWIN,
            // 字母 A-Z（枚举值连续）
            d if (KeyCode::A as u32..=KeyCode::Z as u32).contains(&d) => {
                0x41 + (d - KeyCode::A as u32)
            }
            // 数字 0-9
            d if (KeyCode::Num0 as u32..=KeyCode::Num9 as u32).contains(&d) => {
                0x30 + (d - KeyCode::Num0 as u32)
            }
            // 功能键 F1-F12
            d if (KeyCode::F1 as u32..=KeyCode::F12 as u32).contains(&d) => {
                0x70 + (d - KeyCode::F1 as u32)
            }
            // 方向键
            d if d == KeyCode::Left as u32 => VK_LEFT,
            d if d == KeyCode::Right as u32 => VK_RIGHT,
            d if d == KeyCode::Up as u32 => VK_UP,
            d if d == KeyCode::Down as u32 => VK_DOWN,
            // 导航键
            d if d == KeyCode::Home as u32 => VK_HOME,
            d if d == KeyCode::End as u32 => VK_END,
            d if d == KeyCode::PageUp as u32 => VK_PRIOR,
            d if d == KeyCode::PageDown as u32 => VK_NEXT,
            // 编辑键
            d if d == KeyCode::Enter as u32 => VK_RETURN,
            d if d == KeyCode::Escape as u32 => VK_ESCAPE,
            d if d == KeyCode::Backspace as u32 => VK_BACK,
            d if d == KeyCode::Delete as u32 => VK_DELETE,
            d if d == KeyCode::Tab as u32 => VK_TAB,
            d if d == KeyCode::Space as u32 => VK_SPACE,
            d if d == KeyCode::Insert as u32 => VK_INSERT,
            _ => return false,
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
