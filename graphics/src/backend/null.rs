//! 空渲染后端 — 测试桩，所有绘制均为 no-op。

use uix_platform::{Error, Point, Rect};

use crate::backend::traits::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
use crate::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::traits::Canvas2D;

/// Null DrawSurface。
struct NullDrawSurface {
    canvas: NoopCanvas2D,
}

impl DrawSurface for NullDrawSurface {
    fn size(&self) -> uix_platform::Size {
        self.canvas.surface_size()
    }

    fn push_clip(&mut self, _rect: Rect) {}
    fn pop_clip(&mut self) {}
    fn clear_all(&mut self) {}
    fn clear_rect_raw(&mut self, _x: i32, _y: i32, _w: i32, _h: i32) {}
    fn copy_region(&mut self, _src: Rect, _dst: Point) {}

    fn canvas(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas
    }
}

/// 空渲染后端。
pub struct NullBackend {
    surface: NullDrawSurface,
}

impl NullBackend {
    pub fn new() -> Self {
        Self {
            surface: NullDrawSurface {
                canvas: NoopCanvas2D,
            },
        }
    }
}

impl Default for NullBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderBackend for NullBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Null
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::null()
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn shutdown(&mut self) {}

    fn surface(&mut self) -> &mut dyn DrawSurface {
        &mut self.surface
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::RenderOutcome;
    use crate::pipeline::frame;
    use crate::traits::UpdateStrategy;

    #[test]
    fn null_backend_kind_and_capabilities() {
        let backend = NullBackend::new();
        assert_eq!(backend.kind(), BackendKind::Null);
        assert!(backend.capabilities().partial_redraw);
        assert!(!backend.capabilities().offscreen);
    }

    #[test]
    fn null_backend_begin_frame_idle_on_empty_dirty() {
        let mut backend = NullBackend::new();
        let caps = backend.capabilities();
        let outcome = frame::begin_frame(
            UpdateStrategy::DirtyRects(vec![]),
            backend.surface(),
            800,
            600,
            caps,
        );
        assert_eq!(outcome, RenderOutcome::Idle);
    }

    #[test]
    fn null_backend_canvas_ops_no_panic() {
        let mut backend = NullBackend::new();
        let canvas = backend.surface().canvas();
        canvas.fill_rect(
            Rect::new(0.0, 0.0, 10.0, 10.0),
            crate::color::Color::from_rgba(255, 0, 0, 255),
            None,
        );
    }
}
