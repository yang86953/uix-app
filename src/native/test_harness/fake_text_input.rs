//! Fake 文字输入 — 记录 IME 激活状态变化。

use crate::core::{Rect, Result};
use crate::native::traits::input::ITextInput;

#[derive(Debug, Clone, Default)]
pub struct FakeTextInputState {
    pub active: bool,
    pub start_calls: usize,
    pub stop_calls: usize,
    pub cursor_rect: Option<Rect>,
}

#[derive(Debug)]
pub struct FakeTextInput {
    pub state: FakeTextInputState,
}

impl FakeTextInput {
    pub fn new() -> Self {
        Self {
            state: FakeTextInputState::default(),
        }
    }

    pub fn clear_history(&mut self) {
        self.state.start_calls = 0;
        self.state.stop_calls = 0;
    }
}

impl Default for FakeTextInput {
    fn default() -> Self {
        Self::new()
    }
}

impl ITextInput for FakeTextInput {
    fn start(&mut self) -> Result<()> {
        self.state.active = true;
        self.state.start_calls += 1;
        Ok(())
    }

    fn stop(&mut self) -> Result<()> {
        self.state.active = false;
        self.state.stop_calls += 1;
        Ok(())
    }

    fn set_cursor_rect(&mut self, rect: Rect) -> Result<()> {
        self.state.cursor_rect = Some(rect);
        Ok(())
    }
}
