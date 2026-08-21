// ============================================================================
// native/backends/windows/keyboard.rs — Windows keyboard input (IKeyboard)
// ============================================================================

#![cfg(windows)]
#![allow(clippy::upper_case_acronyms)]

use crate::platform::windowing::{IKeyboard, KeyCode};

// Windows 键盘后端只在 crate 内部平台注册表中构造。
pub(crate) struct WindowsKeyboard;

impl WindowsKeyboard {
    // 创建无状态 Windows 键盘后端。
    pub(crate) fn new() -> Self {
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
        virtual_key_candidates(key)
            .into_iter()
            .filter(|virtual_key| *virtual_key != 0)
            .any(is_key_down)
    }

    fn idle_ms(&self) -> u32 {
        // SAFETY: LASTINPUTINFO 的尺寸字段和可写地址有效，GetTickCount 不接收指针且两次调用均在当前线程同步完成。
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
        // SAFETY: GetDoubleClickTime 无参数，只读取系统双击时间配置。
        unsafe { GetDoubleClickTime() }
    }
}

pub(crate) fn virtual_key_candidates(key: KeyCode) -> [u32; 2] {
    let disc = key as u32;
    let primary = match disc {
        // 修饰键
        d if d == KeyCode::Shift as u32 => VK_SHIFT,
        d if d == KeyCode::Ctrl as u32 => VK_CONTROL,
        d if d == KeyCode::Alt as u32 => VK_MENU,
        d if d == KeyCode::Super as u32 => VK_LWIN,
        // 字母 A-Z（枚举值连续）
        d if (KeyCode::A as u32..=KeyCode::Z as u32).contains(&d) => 0x41 + (d - KeyCode::A as u32),
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
        _ => 0,
    };
    let secondary = if key == KeyCode::Super { VK_RWIN } else { 0 };
    [primary, secondary]
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
const VK_RWIN: u32 = 0x5C;
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

use super::ffi::GetAsyncKeyState;

fn is_key_down(vk: u32) -> bool {
    // SAFETY: GetAsyncKeyState 接收按值传递的虚拟键码，不借用任何 Rust 内存。
    unsafe { GetAsyncKeyState(vk as i32) < 0 }
}

#[link(name = "user32")]
// SAFETY: 声明与 user32 ABI 一致，调用方为 LASTINPUTINFO 提供正确尺寸并只传入有效虚拟键码。
unsafe extern "system" {
    fn GetLastInputInfo(plii: *mut LASTINPUTINFO) -> i32;
    fn GetTickCount() -> u32;
    fn GetDoubleClickTime() -> u32;
}
