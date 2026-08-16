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
        // 未接线协议能力必须返回 typed error，禁止把日志冒充平台动作。
        Err(Error::new(
            // 使用 NotImplemented 表达当前 Adapter 的稳定能力缺失。
            Errc::NotImplemented,
            // 保留 operation、请求形状与待接线协议上下文。
            format!(
                // 诊断不得声称 compositor 已应用光标。
                "WaylandBackend::set_cursor({name}): wp_cursor_shape_manager_v1 is not wired"
            ),
        ))
    }
    fn show_cursor(&mut self, visible: bool) -> Result<()> {
        // 可见性同样不能以 compositor-controlled 日志伪装成功。
        Err(Error::new(
            // 使用相同 capability 分类供 App 交接层稳定去重。
            Errc::NotImplemented,
            // 保留 operation 与请求值，便于后续协议实现定位。
            format!(
                // 诊断明确当前缺少 cursor surface/shape 可见性接线。
                "WaylandBackend::show_cursor({visible}): Wayland cursor visibility is not wired"
            ),
        ))
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
