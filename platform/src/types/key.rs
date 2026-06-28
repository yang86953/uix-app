// ============================================================================
// platform/types/key.rs — 按键码、修饰键与键盘状态查询
//
// 跨层共享（平台输入 → UI 事件）。
// ============================================================================

// ════════════════════════════════════════════════════════════════════════════
// IKeyboard — 键盘状态查询
// ════════════════════════════════════════════════════════════════════════════

pub trait IKeyboard {
    fn is_down(&self, key: KeyCode) -> bool;
    fn idle_ms(&self) -> u32;
    fn double_click_ms(&self) -> u32;
}

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

/// 修饰键掩码 — 跨层共享。
///
/// 手动实现位标志，替代 bitflags crate。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyMod(u32);

#[allow(non_upper_case_globals)]
impl KeyMod {
    pub const NONE: Self  = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self  = Self(1 << 1);
    pub const ALT: Self   = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);

    /// 检查是否包含指定标志的所有位。
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    /// 检查是否包含指定标志的任意位。
    pub const fn intersects(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }
}

impl std::ops::BitOr for KeyMod {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl std::ops::BitOrAssign for KeyMod {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}
