//! 输入协议 — 键盘、鼠标、光标、剪贴板与 IME。

use crate::core::error::Result;
use crate::core::geometry::{Point, Rect};

// ════════════════════════════════════════════════════════════════════════════
// 鼠标按钮 — 跨层共享
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

// ════════════════════════════════════════════════════════════════════════════
// 光标类型
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum CursorType {
    Arrow,
    IBeam,
    Crosshair,
    Hand,
    ResizeH,
    ResizeV,
    ResizeNE,
    ResizeNW,
    Move,
    Wait,
    NotAllowed,
    Custom,
}

// ════════════════════════════════════════════════════════════════════════════
// 按键码 — 跨层共享（平台输入 → UI 事件）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum KeyCode {
    Unknown = 0,
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
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    PageUp,
    PageDown,
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Space,
    Insert,
    Shift,
    Ctrl,
    Alt,
    Super,
}

// ════════════════════════════════════════════════════════════════════════════
// 修饰键掩码 — 跨层共享
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyMod(u32);

#[allow(non_upper_case_globals)]
impl KeyMod {
    pub const NONE: Self = Self(0);
    pub const SHIFT: Self = Self(1 << 0);
    pub const CTRL: Self = Self(1 << 1);
    pub const ALT: Self = Self(1 << 2);
    pub const SUPER: Self = Self(1 << 3);
    /// 框架内部合成的按键事件；平台 backend 不得设置。
    pub(crate) const SYNTHETIC: Self = Self(1 << 31);

    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

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

// ════════════════════════════════════════════════════════════════════════════
// 控件尺寸与滚动方向
// ════════════════════════════════════════════════════════════════════════════

/// 通用控件尺寸。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSize {
    Small,
    Medium,
    Large,
}

/// 滚动方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollDirection {
    /// 仅纵向滚动。
    Vertical,
    /// 仅横向滚动。
    Horizontal,
    /// 双向滚动。
    Both,
}

impl ScrollDirection {
    pub fn can_scroll_x(&self) -> bool {
        matches!(self, Self::Horizontal | Self::Both)
    }

    pub fn can_scroll_y(&self) -> bool {
        matches!(self, Self::Vertical | Self::Both)
    }
}

pub trait IClipboard {
    fn text(&self) -> Result<String>;
    fn set_text(&mut self, text: &str) -> Result<()>;
    fn has_text(&self) -> Result<bool>;
}

pub trait ICursor {
    fn set_cursor(&mut self, cursor: CursorType) -> Result<()>;
    fn show_cursor(&mut self, visible: bool) -> Result<()>;
    fn cursor_position(&self) -> Result<Point>;
    fn set_cursor_position(&mut self, x: i32, y: i32) -> Result<()>;
    fn confine_cursor(&mut self, confine: bool) -> Result<()>;
    fn capture_mouse(&mut self) -> Result<()>;
    fn release_mouse(&mut self) -> Result<()>;
}

pub trait ITextInput {
    /// Select the native window that owns the following IME session calls.
    ///
    /// Text input is window-scoped even when a platform exposes one process-wide
    /// IME service. Implementations must switch both the native target and the
    /// `WindowId` used to tag emitted events as one operation.
    fn set_target_window(
        &mut self,
        window_id: crate::core::WindowId,
        native_window: *mut std::ffi::c_void,
    ) -> Result<()>;
    fn start(&mut self) -> Result<()>;
    fn stop(&mut self) -> Result<()>;
    fn set_cursor_rect(&mut self, rect: Rect) -> Result<()>;
}

pub trait IKeyboard {
    fn is_down(&self, key: KeyCode) -> bool;
    fn idle_ms(&self) -> u32;
    fn double_click_ms(&self) -> u32;
}
