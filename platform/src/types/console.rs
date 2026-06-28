// ============================================================================
// platform/types/console.rs — 终端 API 契约与类型
// ============================================================================

// ════════════════════════════════════════════════════════════════════════════
// IConsole — 终端控制台
// ════════════════════════════════════════════════════════════════════════════

pub trait IConsole {
    fn write(&mut self, text: &str);
    fn write_line(&mut self, text: &str);
    fn set_color(&mut self, color: ConsoleColor);
    fn reset_color(&mut self);
    fn show_terminal_cursor(&mut self, visible: bool);
    fn set_terminal_title(&mut self, title: &str);
    fn capabilities(&self) -> TerminalCapabilities;
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
