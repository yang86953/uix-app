// ============================================================================
// platform/linux/wayland/cursor.rs — ICursor impl for WaylandBackend
// ============================================================================

use crate::platform::api::input::CursorType;
use crate::platform::ICursor;
use crate::platform::Point;

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
        crate::platform::log::trace_fn(format!("Wayland: set_cursor({}) requested", name));
    }
    fn show_cursor(&mut self, visible: bool) {
        crate::platform::log::debug_fn(format!(
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
