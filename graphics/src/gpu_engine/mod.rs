// ============================================================================
// graphics/gpu_engine/mod.rs — GPU 渲染引擎（v2 重构后，暂时 stub）
// ============================================================================

use std::cell::RefCell;
use glow::HasContext as _;
use uix_core::{Rect, Size};
use uix_diag::Error;
use uix_platform::api::IGraphicsContext;
use crate::engine::cpu::noop_canvas_2d::NoopCanvas2D;
use crate::engine::cpu::noop_canvas_3d::NoopCanvas3D;
use crate::traits::{Canvas2D, Canvas3D, GraphicsEngine, UpdateStrategy};
use crate::engine::RenderOutcome;

pub use shaders::*;
mod shaders;

pub struct GpuEngine {
    pub gl: glow::Context,
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    pub width: i32,
    pub height: i32,
    pub(crate) readback: RefCell<Vec<u32>>,
    canvas_2d: NoopCanvas2D,
    canvas_3d: NoopCanvas3D,
}

impl GpuEngine {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Self {
        let gl = unsafe {
            glow::Context::from_loader_function(|s| gpu_ctx.get_proc_address(s).unwrap_or(std::ptr::null()))
        };
        Self {
            gl,
            gpu_ctx,
            width: 0,
            height: 0,
            readback: RefCell::new(Vec::new()),
            canvas_2d: NoopCanvas2D,
            canvas_3d: NoopCanvas3D,
        }
    }

    pub fn pixels(&self) -> Vec<u32> {
        self.readback.borrow().clone()
    }
}

impl GraphicsEngine for GpuEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.width = w;
        self.height = h;
        unsafe { self.gl.viewport(0, 0, w, h); }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.gpu_ctx.shutdown();
    }

    fn resize(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        self.gpu_ctx.resize(w, h);
        unsafe { self.gl.viewport(0, 0, w, h); }
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        self.gpu_ctx.make_current();
        RenderOutcome::Present(None)
    }

    fn end_frame(&mut self) -> RenderOutcome {
        unsafe { self.gl.flush(); }
        RenderOutcome::Present(None)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas_2d
    }

    fn canvas_3d(&mut self) -> &mut dyn Canvas3D {
        &mut self.canvas_3d
    }
}
