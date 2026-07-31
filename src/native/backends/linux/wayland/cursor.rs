// ============================================================================
// platform/linux/wayland/cursor.rs — ICursor impl for WaylandBackend
// ============================================================================

use crate::core::Point;
use crate::native::windowing::input::CursorType;
use crate::native::windowing::input::ICursor;
use crate::native::{Errc, Error, Result};

use super::WaylandBackend;

impl ICursor for WaylandBackend {
    fn set_cursor(&mut self, cursor: CursorType) -> Result<()> {
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
        tracing::trace!("Wayland: set_cursor({}) requested", name);
        Ok(())
    }
    fn show_cursor(&mut self, visible: bool) -> Result<()> {
        tracing::debug!("Wayland: show_cursor({}) — compositor-controlled", visible);
        Ok(())
    }
    fn cursor_position(&self) -> Result<Point> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::cursor_position: not provided by wl_seat",
        ))
    }
    fn set_cursor_position(&mut self, _: i32, _: i32) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::set_cursor_position: compositor-controlled",
        ))
    }
    fn confine_cursor(&mut self, _: bool) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::confine_cursor: compositor-controlled",
        ))
    }
    fn capture_mouse(&mut self) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::capture_mouse: compositor-controlled",
        ))
    }
    fn release_mouse(&mut self) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "WaylandBackend::release_mouse: compositor-controlled",
        ))
    }
}
