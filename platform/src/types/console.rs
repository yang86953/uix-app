// ============================================================================
// platform/types/console.rs — 终端类型
//
// IConsole trait 已迁移至 crate::api::traits。
// ============================================================================

// ════════════════════════════════════════════════════════════════════════════
// 终端颜色 / 能力
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
