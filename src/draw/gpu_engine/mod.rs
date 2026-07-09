// ============================================================================
// draw/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

use glow::HasContext as _;
use std::cell::RefCell;

use crate::core::Error;
use crate::draw::backend::registry::create_native_raster_backend;
use crate::draw::backend::{DamageRegion, RenderBackend};
use crate::draw::engine::RenderOutcome;
use crate::draw::pipeline::RenderSession;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::IGraphicsContext;

pub use canvas_2d::GpuCanvas2D;
pub use shaders::*;
mod canvas_2d;
mod shaders;

/// GPU 渲染引擎 — 委托 `RenderSession` + `GpuBackend`。
pub struct GpuEngine {
    session: RenderSession,
    empty_readback: RefCell<Vec<u32>>,
}

impl GpuEngine {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match create_native_raster_backend(gpu_ctx) {
            Ok(backend) => RenderSession::with_backend(backend),
            Err(err) => return Err(err),
        };
        Ok(Self {
            session,
            empty_readback: RefCell::new(Vec::new()),
        })
    }

    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }

    /// 返回像素缓冲的克隆（每次调用分配，仅用于读回）。
    pub fn pixels(&self) -> Vec<u32> {
        self.session
            .gpu_backend()
            .map(|g| g.pixels())
            .unwrap_or_default()
    }

    /// 返回像素缓冲的引用（避免分配）。
    pub fn pixels_ref(&self) -> std::cell::Ref<'_, Vec<u32>> {
        if let Some(gpu) = self.session.gpu_backend() {
            gpu.pixels_ref()
        } else {
            self.empty_readback.borrow()
        }
    }

    /// 从 GL 前端缓冲读回像素数据（填充 readback）。
    pub fn read_pixels(&self) {
        if let Some(gpu) = self.session.gpu_backend() {
            gpu.read_pixels();
        }
    }
}

impl GraphicsEngine for GpuEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.initialize(w, h)?;
        if let Some(gpu) = self.session.gpu_backend_mut() {
            gpu.gpu_ctx.make_current();
            let vw = gpu.gpu_ctx.width();
            let vh = gpu.gpu_ctx.height();
            unsafe {
                gpu.gl.viewport(0, 0, vw, vh);
                gpu.gl.enable(glow::BLEND);
                gpu.gl
                    .blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            }
        }
        Ok(())
    }

    fn shutdown(&mut self) {
        self.session.shutdown();
    }

    fn resize(&mut self, w: i32, h: i32) {
        self.session.resize(w, h);
        if let Some(gpu) = self.session.gpu_backend_mut() {
            gpu.gpu_ctx.make_current();
            unsafe {
                gpu.gl
                    .viewport(0, 0, gpu.gpu_ctx.width(), gpu.gpu_ctx.height());
            }
        }
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        if let Some(gpu) = self.session.gpu_backend_mut() {
            gpu.gpu_ctx.make_current();
        }
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if let Some(gpu) = self.session.gpu_backend_mut() {
            if gpu.present(present_damage).is_err() {
                crate::core::log::error_fn("GpuEngine present 失败");
            }
        }
        outcome
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.session.graphics_capabilities()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.session
            .gpu_backend()
            .map(|gpu| gpu.gpu_ctx.device_pixel_ratio())
            .unwrap_or(1.0)
    }
}
