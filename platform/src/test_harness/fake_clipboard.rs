//! Fake 剪贴板 — 内存实现 + 操作记录。

use crate::api::traits::IClipboard;
use std::cell::Cell;

/// 剪贴板状态（公开字段，测试可直接读取断言）
#[derive(Debug, Clone)]
pub struct FakeClipboardState {
    /// 当前剪贴板文本
    pub text: String,
    /// `set_text` 调用历史
    pub set_text_calls: Vec<String>,
    /// `has_text` 调用次数（通过 Cell 支持 &self 追踪）
    pub has_text_calls: Cell<usize>,
}

impl Default for FakeClipboardState {
    fn default() -> Self {
        Self {
            text: String::new(),
            set_text_calls: Vec::new(),
            has_text_calls: Cell::new(0),
        }
    }
}

/// 内存剪贴板。写入后读出一致，支持断言调用历史。
#[derive(Debug)]
pub struct FakeClipboard {
    pub state: FakeClipboardState,
}

impl FakeClipboard {
    pub fn new() -> Self {
        Self {
            state: FakeClipboardState::default(),
        }
    }

    /// 最后一次 `set_text` 的内容
    pub fn last_set_text(&self) -> Option<&str> {
        self.state.set_text_calls.last().map(|s| s.as_str())
    }

    /// 清除调用记录（保留当前文本）
    pub fn clear_history(&mut self) {
        self.state.set_text_calls.clear();
        self.state.has_text_calls.set(0);
    }
}

impl IClipboard for FakeClipboard {
    fn text(&self) -> String {
        self.state.text.clone()
    }

    fn set_text(&mut self, text: &str) {
        self.state.text = text.to_string();
        self.state.set_text_calls.push(text.to_string());
    }

    fn has_text(&self) -> bool {
        self.state
            .has_text_calls
            .set(self.state.has_text_calls.get() + 1);
        !self.state.text.is_empty()
    }
}
