// ============================================================================
// platform/types.rs — Platform 层基础共享类型
// ============================================================================
//
// 仅保留跨子系统共享的纯基础类型。子系统专用类型（CursorType、DisplayInfo、
// 事件载荷结构体等）已迁至各自的 trait 文件中。
// ============================================================================

use bitflags::bitflags;

// ════════════════════════════════════════════════════════════════════════════
// 几何类型 — 从 graphics 模块统一引入，避免重复定义
// ════════════════════════════════════════════════════════════════════════════

pub use crate::base::{EdgeInsets, Point, Rect, Size};

// ════════════════════════════════════════════════════════════════════════════
// 按键码（被 event、input、cursor 等多个子系统共享）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KeyCode {
    // 未知/未映射的按键
    Unknown = 0,

    // 字母 A-Z
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // 数字 0-9
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,

    // 功能键 F1-F12
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,

    // 方向键
    Up,
    Down,
    Left,
    Right,

    // 导航键
    Home,
    End,
    PageUp,
    PageDown,

    // 编辑键
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Space,
    Insert,

    // 修饰键
    Shift,
    Ctrl,
    Alt,
    Super,
}

// ════════════════════════════════════════════════════════════════════════════
// 修饰键掩码
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
// 鼠标按钮（被 event、cursor 等多个子系统共享）
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
