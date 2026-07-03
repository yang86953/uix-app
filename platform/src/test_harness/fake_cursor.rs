//! Fake 光标 — 追踪光标位置、可见性、捕获状态。

use crate::api::traits::ICursor;
use crate::types::CursorType;
use crate::Point;

#[derive(Debug, Clone)]
pub struct FakeCursorState {
    pub cursor_type: CursorType,
    pub visible: bool,
    pub position: Point,
    pub confined: bool,
    pub captured: bool,
    /// `set_cursor` 调用历史
    pub set_cursor_calls: Vec<CursorType>,
    /// `set_cursor_position` 调用历史
    pub set_position_calls: Vec<(i32, i32)>,
    pub show_cursor_calls: Vec<bool>,
    pub confine_calls: Vec<bool>,
    pub capture_calls: usize,
    pub release_calls: usize,
}

impl Default for FakeCursorState {
    fn default() -> Self {
        Self {
            cursor_type: CursorType::Arrow,
            visible: true,
            position: Point::zero(),
            confined: false,
            captured: false,
            set_cursor_calls: Vec::new(),
            set_position_calls: Vec::new(),
            show_cursor_calls: Vec::new(),
            confine_calls: Vec::new(),
            capture_calls: 0,
            release_calls: 0,
        }
    }
}

#[derive(Debug)]
pub struct FakeCursor {
    pub state: FakeCursorState,
}

impl FakeCursor {
    pub fn new() -> Self {
        Self { state: FakeCursorState::default() }
    }

    pub fn clear_history(&mut self) {
        self.state.set_cursor_calls.clear();
        self.state.set_position_calls.clear();
        self.state.show_cursor_calls.clear();
        self.state.confine_calls.clear();
        self.state.capture_calls = 0;
        self.state.release_calls = 0;
    }
}

impl ICursor for FakeCursor {
    fn set_cursor(&mut self, cursor: CursorType) {
        self.state.cursor_type = cursor;
        self.state.set_cursor_calls.push(cursor);
    }

    fn show_cursor(&mut self, visible: bool) {
        self.state.visible = visible;
        self.state.show_cursor_calls.push(visible);
    }

    fn cursor_position(&self) -> Point {
        self.state.position
    }

    fn set_cursor_position(&mut self, x: i32, y: i32) {
        self.state.position = Point::new(x as f32, y as f32);
        self.state.set_position_calls.push((x, y));
    }

    fn confine_cursor(&mut self, confine: bool) {
        self.state.confined = confine;
        self.state.confine_calls.push(confine);
    }

    fn capture_mouse(&mut self) {
        self.state.captured = true;
        self.state.capture_calls += 1;
    }

    fn release_mouse(&mut self) {
        self.state.captured = false;
        self.state.release_calls += 1;
    }
}
