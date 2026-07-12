//! 空渲染后端 — 测试桩，所有绘制均为 no-op。

use crate::core::{Error, Point, Rect};

use crate::draw::backend::traits::{BackendCapabilities, BackendKind, DrawSurface, RenderBackend};
use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::pipeline::{EncodedFrameExecution, FrameEncoder};
use crate::draw::traits::Canvas2D;

/// Null DrawSurface。
struct NullDrawSurface {
    canvas: NoopCanvas2D,
}

impl DrawSurface for NullDrawSurface {
    fn size(&self) -> crate::core::Size {
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

    fn try_execute_encoded_frame(
        &mut self,
        _encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        Ok(EncodedFrameExecution::Executed)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[cfg(test)]
#[path = "../../tests/draw/backend/null.rs"]
mod tests;
