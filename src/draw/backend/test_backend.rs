//! 测试后端 — 所有绘制均为 no-op。

use crate::core::{Error, Point, Rect};

use crate::draw::backend::contract::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::command::{EncodedFrameExecution, FrameEncoder};
use crate::draw::Canvas2D;

/// Test-only no-op DrawSurface.
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
pub(crate) struct TestBackend {
    surface: NullDrawSurface,
}

impl TestBackend {
    pub fn new() -> Self {
        Self {
            surface: NullDrawSurface {
                canvas: NoopCanvas2D,
            },
        }
    }
}

impl Default for TestBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl RenderBackend for TestBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Test
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities::test()
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        Ok(())
    }

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
