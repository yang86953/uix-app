// ============================================================================
// graphics/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

use glow::HasContext as _;
use std::cell::RefCell;

use crate::engine::RenderOutcome;
use crate::traits::{Canvas2D, GraphicsEngine, UpdateStrategy};
use uix_platform::Error;
use uix_platform::IGraphicsContext;

pub use canvas_2d::GpuCanvas2D;
pub use shaders::*;
mod canvas_2d;
mod shaders;

pub struct GpuEngine {
    /// 堆分配的 glow::Context，确保 GpuCanvas2D 中的 gl_ptr 不受 move 影响。
    pub gl: Box<glow::Context>,
    pub gpu_ctx: Box<dyn IGraphicsContext>,
    pub width: i32,
    pub height: i32,
    pub(crate) readback: RefCell<Vec<u32>>,
    canvas_2d: GpuCanvas2D,
}

impl GpuEngine {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Self {
        // Box::new 将 Context 分配在堆上，地址固定。
        // GpuCanvas2D 中的 gl_ptr 指向此堆地址，不受 Self move 影响。
        let gl = Box::new(unsafe {
            glow::Context::from_loader_function(|s| {
                gpu_ctx.get_proc_address(s).unwrap_or(std::ptr::null())
            })
        });
        let canvas_2d = GpuCanvas2D::new(&gl, 1, 1);
        Self {
            gl,
            gpu_ctx,
            width: 0,
            height: 0,
            readback: RefCell::new(Vec::new()),
            canvas_2d,
        }
    }

    /// 返回像素缓冲的克隆（每次调用分配，仅用于读回）。
    pub fn pixels(&self) -> Vec<u32> {
        self.readback.borrow().clone()
    }

    /// 返回像素缓冲的引用（避免分配）。
    pub fn pixels_ref(&self) -> std::cell::Ref<'_, Vec<u32>> {
        self.readback.borrow()
    }

    /// 从 GL 前端缓冲读回像素数据（填充 readback）。
    pub fn read_pixels(&self) {
        let mut rb = self.readback.borrow_mut();
        let len = (self.width * self.height * 4) as usize;
        if rb.len() < len {
            rb.resize(len, 0);
        }
        unsafe {
            self.gl.read_pixels(
                0,
                0,
                self.width,
                self.height,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelPackData::Slice(Some(std::slice::from_raw_parts_mut(
                    rb.as_mut_ptr() as *mut u8,
                    len,
                ))),
            );
        }
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
            self.gl
                .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
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
}
