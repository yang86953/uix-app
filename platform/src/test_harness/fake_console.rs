//! Fake 控制台 — 记录所有输出到内存缓冲区，支持断言。

use crate::api::traits::IConsole;
use crate::types::{ConsoleColor, TerminalCapabilities};

/// 控制台状态（公开字段，测试可直接读取断言）
#[derive(Debug, Clone)]
pub struct FakeConsoleState {
    /// 累积的纯文本输出（不含控制序列）
    pub output: String,
    /// 每次 `write` 的内容
    pub writes: Vec<String>,
    /// 每次 `write_line` 的内容（不含换行符）
    pub lines: Vec<String>,
    /// `set_color` 调用历史
    pub colors: Vec<ConsoleColor>,
    /// 当前颜色
    pub current_color: ConsoleColor,
    /// 光标是否可见
    pub cursor_visible: bool,
    /// 终端标题
    pub title: String,
    /// 终端能力
    pub capabilities: TerminalCapabilities,
}

impl Default for FakeConsoleState {
    fn default() -> Self {
        Self {
            output: String::new(),
            writes: Vec::new(),
            lines: Vec::new(),
            colors: Vec::new(),
            current_color: ConsoleColor::Default,
            cursor_visible: true,
            title: String::new(),
            capabilities: TerminalCapabilities {
                has_color: true,
                has_raw_mode: false,
                has_cursor_control: true,
            },
        }
    }
}

#[derive(Debug)]
pub struct FakeConsole {
    pub state: FakeConsoleState,
}

impl FakeConsole {
    pub fn new() -> Self {
        Self { state: FakeConsoleState::default() }
    }

    /// 最后一行输出
    pub fn last_line(&self) -> Option<&str> {
        self.state.lines.last().map(|s| s.as_str())
    }

    /// 最后一次设置的颜色
    pub fn last_color(&self) -> Option<ConsoleColor> {
        self.state.colors.last().copied()
    }

    /// 输出是否包含指定文本
    pub fn output_contains(&self, substr: &str) -> bool {
        self.state.output.contains(substr)
    }

    pub fn clear_history(&mut self) {
        self.state.writes.clear();
        self.state.lines.clear();
        self.state.colors.clear();
        self.state.output.clear();
    }
}

impl IConsole for FakeConsole {
    fn write(&mut self, text: &str) {
        self.state.output.push_str(text);
        self.state.writes.push(text.to_string());
    }

    fn write_line(&mut self, text: &str) {
        self.state.output.push_str(text);
        self.state.output.push('\n');
        self.state.writes.push(text.to_string());
        self.state.lines.push(text.to_string());
    }

    fn set_color(&mut self, color: ConsoleColor) {
        self.state.current_color = color;
        self.state.colors.push(color);
    }

    fn reset_color(&mut self) {
        self.state.current_color = ConsoleColor::Default;
        self.state.colors.push(ConsoleColor::Default);
    }

    fn show_terminal_cursor(&mut self, visible: bool) {
        self.state.cursor_visible = visible;
    }

    fn set_terminal_title(&mut self, title: &str) {
        self.state.title = title.to_string();
    }

    fn capabilities(&self) -> TerminalCapabilities {
        self.state.capabilities.clone()
    }
}
