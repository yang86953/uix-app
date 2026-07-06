// ============================================================================
// platform/linux/wayland/cursor.rs — ICursor impl for WaylandBackend
// ============================================================================

use crate::core::Point;
use crate::native::traits::input::CursorType;
use crate::native::traits::input::ICursor;

use super::WaylandBackend;

impl ICursor for WaylandBackend {
    fn set_cursor(&mut self, cursor: CursorType) {
        let name = match cursor {
            CursorType::Arrow => "default",
            CursorType::IBeam => "text",
            CursorType::Crosshair => "crosshair",
            CursorType::Hand => "pointer",
            CursorType::ResizeH => "ew-resize",
            CursorType::ResizeV => "ns-resize",
            CursorType::ResizeNE => "ne-resize",
            CursorType::ResizeNW => "nw-resize",
            CursorType::Move => "move",
            CursorType::Wait => "wait",
            CursorType::NotAllowed => "not-allowed",
            CursorType::Custom => "default",
        };
        crate::core::log::trace_fn(format!("Wayland: set_cursor({}) requested", name));
    }
    fn show_cursor(&mut self, visible: bool) {
        crate::core::log::debug_fn(format!(
            "Wayland: show_cursor({}) — compositor-controlled",
            visible
        ));
    }
    fn cursor_position(&self) -> Point {
        Point::default()
    }
    fn set_cursor_position(&mut self, _: i32, _: i32) {}
    fn confine_cursor(&mut self, _: bool) {}
    fn capture_mouse(&mut self) {}
    fn release_mouse(&mut self) {}
}
