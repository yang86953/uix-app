// ============================================================================
// platform/types/input.rs — 输入相关类型与 API 契约
//
// 跨层共享的鼠标按钮、光标类型、剪贴板、光标控制、文本输入。
// ============================================================================

use crate::geometry::Point;

// ════════════════════════════════════════════════════════════════════════════
// IClipboard — 剪贴板
// ════════════════════════════════════════════════════════════════════════════

pub trait IClipboard {
    fn text(&self) -> String;
    fn set_text(&mut self, text: &str);
    fn has_text(&self) -> bool;
}

// ════════════════════════════════════════════════════════════════════════════
// ICursor — 光标
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

// ════════════════════════════════════════════════════════════════════════════
// ITextInput — 文本输入（IME）控制
// ════════════════════════════════════════════════════════════════════════════

pub trait ITextInput {
    fn start(&mut self);
    fn stop(&mut self);
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
