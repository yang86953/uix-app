// ============================================================================
// graphics/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

use std::cell::RefCell;
use glow::HasContext as _;

use uix_diag::Error;
use uix_platform::api::IGraphicsContext;
use crate::engine::cpu::noop_canvas_3d::NoopCanvas3D;
use crate::traits::{Canvas2D, Canvas3D, GraphicsEngine, UpdateStrategy};
use crate::engine::RenderOutcome;

pub use shaders::*;
pub use canvas_2d::GpuCanvas2D;
mod shaders;
mod canvas_2d;

pub struct GpuEngine {
    pub gl: glow::Context,
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    pub width: i32,
    pub height: i32,
    pub(crate) readback: RefCell<Vec<u32>>,
    canvas_2d: GpuCanvas2D,
    canvas_3d: NoopCanvas3D,
}

impl GpuEngine {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Self {
        let gl = unsafe {
            glow::Context::from_loader_function(|s| gpu_ctx.get_proc_address(s).unwrap_or(std::ptr::null()))
        };
        // 先创建 canvas_2d，再移动 gl 到 self.gl
        let canvas_2d = GpuCanvas2D::new(&gl, 1, 1);
        Self {
            gl,
            gpu_ctx,
            width: 0,
            height: 0,
            readback: RefCell::new(Vec::new()),
            canvas_2d,
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
        self.canvas_2d = GpuCanvas2D::new(&self.gl, w, h);
        unsafe {
            self.gl.viewport(0, 0, w, h);
            self.gl.enable(glow::BLEND);
            self.gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.gpu_ctx.shutdown();
    }

    fn resize(&mut self, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        self.canvas_2d.resize(w, h);
        self.gpu_ctx.resize(w, h);
        unsafe {
            self.gl.viewport(0, 0, w, h);
        }
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.gpu_ctx.make_current();

        // 清除
        if strategy.should_clear() {
            unsafe {
                self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
                self.gl.clear(glow::COLOR_BUFFER_BIT);
            }
        }

        RenderOutcome::Present(None)
    }

    fn end_frame(&mut self) -> RenderOutcome {
        self.gpu_ctx.swap_buffers();
        RenderOutcome::Present(None)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        &mut self.canvas_2d
    }

    fn canvas_3d(&mut self) -> &mut dyn Canvas3D {
        &mut self.canvas_3d
    }
}
