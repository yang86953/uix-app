//! Platform System 的中立控制台合同。

use crate::core::Result;

/// 控制台输出颜色；判别值顺序与各平台颜色表保持一致。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum ConsoleColor {
    Default = 0,
    Trace,
    Debug,
    Info,
    Warn,
    Error,
    Fatal,
}

/// 当前终端能够直接提供的低层能力。
#[derive(Debug, Clone, Copy)]
pub(crate) struct TerminalCapabilities {
    pub(crate) has_color: bool,
    pub(crate) has_raw_mode: bool,
    pub(crate) has_cursor_control: bool,
}

/// Platform 根持有的同步控制台端口。
pub(crate) trait IConsole {
    fn write(&mut self, text: &str) -> Result<()>;
    fn write_line(&mut self, text: &str) -> Result<()>;
    fn set_color(&mut self, color: ConsoleColor) -> Result<()>;
    fn reset_color(&mut self) -> Result<()>;
    fn show_terminal_cursor(&mut self, visible: bool) -> Result<()>;
    fn set_terminal_title(&mut self, title: &str) -> Result<()>;
    fn capabilities(&self) -> TerminalCapabilities;
}
