// ============================================================================
// base/types.rs — 基础共享类型（Layer 0）
//
// 跨层通用的基础数据类型。平台专属类型（CursorType、DisplayInfo 等）
// 定义在 platform/types.rs 中。
// ============================================================================

use bitflags::bitflags;

// ════════════════════════════════════════════════════════════════════════════
// 几何类型 — 从 geometry 模块引入
// ════════════════════════════════════════════════════════════════════════════

pub use crate::base::{EdgeInsets, Point, Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// 按键码 — 跨层共享（平台输入 → UI 事件）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KeyCode {
    Unknown = 0,
    A, B, C, D, E, F, G, H, I, J, K, L, M,
    N, O, P, Q, R, S, T, U, V, W, X, Y, Z,
    Num0, Num1, Num2, Num3, Num4, Num5, Num6, Num7, Num8, Num9,
    F1, F2, F3, F4, F5, F6, F7, F8, F9, F10, F11, F12,
    Up, Down, Left, Right,
    Home, End, PageUp, PageDown,
    Enter, Escape, Backspace, Delete, Tab, Space, Insert,
    Shift, Ctrl, Alt, Super,
}

// ════════════════════════════════════════════════════════════════════════════
// 修饰键掩码 — 跨层共享
// ════════════════════════════════════════════════════════════════════════════

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct KeyMod: u32 {
        const NONE  = 0;
        const SHIFT = 1 << 0;
        const CTRL  = 1 << 1;
        const ALT   = 1 << 2;
        const SUPER = 1 << 3;
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 鼠标按钮 — 跨层共享（平台输入 → UI 事件）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum MouseButton {
    None,
    Left,
    Right,
    Middle,
    X1,
    X2,
}
