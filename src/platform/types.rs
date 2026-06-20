// ============================================================================
// platform/types.rs — 平台层专属数据类型
//
// 平台实现所需的专用枚举和结构体。
// 跨层共享类型（KeyCode、KeyMod、MouseButton）在 crate::base 中定义。
// ============================================================================

use crate::base::Rect;

// ════════════════════════════════════════════════════════════════════════════
// 光标类型（cursor 子系统）
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
}

#[derive(Debug, Clone)]
pub struct OsInfo {
    pub name: String,
    pub version: String,
    pub build: String,
    pub is_64bit: bool,
}


