// ============================================================================
// platform/types.rs — 平台层数据类型
//
// 平台实现所需的专用枚举和结构体，以及跨层共享的输入类型。
// ============================================================================

use bitflags::bitflags;

use crate::Rect;

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
// 光标类型（cursor 子系统）
// ════════════════════════════════════════════════════════════════════════════
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
// 终端颜色 / 能力（console 子系统）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ConsoleColor {
    Default = 0,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

#[derive(Debug, Clone, Copy)]
pub struct TerminalCapabilities {
    pub has_color: bool,
    pub has_raw_mode: bool,
    pub has_cursor_control: bool,
}

// ════════════════════════════════════════════════════════════════════════════
// 显示器信息（display 子系统）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct DisplayInfo {
    pub bounds: Rect,
    pub dpi_scale: f32,
    pub is_primary: bool,
}

impl Default for DisplayInfo {
    fn default() -> Self {
        Self {
            bounds: Rect::default(),
            dpi_scale: 1.0,
            is_primary: false,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 文件系统（file_system 子系统）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SpecialDir {
    Home,
    Temp,
    AppData,
    LocalAppData,
    Documents,
    Desktop,
    Downloads,
    Current,
    Executable,
}

// ════════════════════════════════════════════════════════════════════════════
// 系统信息（system_info 子系统）
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct MemoryInfo {
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub process_working_set: usize,
    pub process_private_bytes: usize,
}

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub is_64bit: bool,
}


