//! NullEngine — 用于测试/演示的 GraphicsEngine 桩实现。
//! 所有渲染操作均为空操作。

use crate::draw::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::draw::engine::RenderOutcome;
use crate::draw::traits::{Canvas2D, GraphicsEngine, UpdateStrategy};
use crate::native::Error;

/// 空图形引擎——不做任何渲染。
pub struct NullEngine {
    canvas_2d: NoopCanvas2D,
}

impl NullEngine {
    pub fn new() -> Self {
        Self {
            canvas_2d: NoopCanvas2D,
        }
    }
}

impl Default for NullEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl GraphicsEngine for NullEngine {
    fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }

    fn shutdown(&mut self) {}

    fn resize(&mut self, _width: i32, _height: i32) {}

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::Idle
    }

    fn end_frame(&mut self) -> RenderOutcome {
        RenderOutcome::Idle
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas_2d
    }
}
