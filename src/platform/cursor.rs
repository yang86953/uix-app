// ============================================================================
// platform/cursor.rs — 光标抽象接口
// ============================================================================

use crate::platform::types::Point;

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
// ICursor — 光标能力接口
// ════════════════════════════════════════════════════════════════════════════

pub trait ICursor {
    fn set_cursor(&mut self, cursor: CursorType);
    fn show_cursor(&mut self, visible: bool);
    fn cursor_position(&self) -> Point;
    fn set_cursor_position(&mut self, x: i32, y: i32);
    fn confine_cursor(&mut self, confine: bool);
    fn capture_mouse(&mut self);
    fn release_mouse(&mut self);
}
