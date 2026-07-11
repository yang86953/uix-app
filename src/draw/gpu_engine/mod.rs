// ============================================================================
// draw/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

use glow::HasContext as _;
use std::cell::RefCell;

use crate::core::Error;
use crate::draw::backend::registry::create_native_raster_backend;
use crate::draw::backend::DamageRegion;
use crate::draw::engine::RenderOutcome;
use crate::draw::pipeline::RenderSession;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::IGraphicsContext;

pub use canvas_2d::GpuCanvas2D;
pub use shaders::*;
mod canvas_2d;
mod shaders;

/// GPU 渲染引擎 — 委托 `RenderSession` + `RenderBackend`（GL / D3D11 / …）。
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

    fn make_current(&mut self) {
        self.session.backend_mut().make_current();
    }
}

impl GraphicsEngine for GpuEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.initialize(w, h)?;
        self.make_current();
        if let Some(gpu) = self.session.gpu_backend_mut() {
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
        self.make_current();
        if let Some(gpu) = self.session.gpu_backend_mut() {
            unsafe {
                gpu.gl
                    .viewport(0, 0, gpu.gpu_ctx.width(), gpu.gpu_ctx.height());
            }
        }
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.make_current();
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if self.session.backend_mut().present(present_damage).is_err() {
            crate::core::log::error_fn("GpuEngine present 失败");
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
        self.session.backend().device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<crate::draw::ImageHandle> {
        self.session.backend_mut().create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: crate::draw::ImageHandle) {
        self.session.backend_mut().destroy_offscreen(handle);
    }

    fn offscreen_canvas(
        &mut self,
        handle: &crate::draw::ImageHandle,
    ) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn copy_offscreen_pixels(
        &self,
        handle: &crate::draw::ImageHandle,
    ) -> Option<(Vec<u32>, i32)> {
        self.session.backend().copy_offscreen_pixels(handle)
    }

    fn begin_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) -> bool {
        self.session.backend_mut().begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) {
        self.session.backend_mut().flush_offscreen_paint(handle);
    }

    fn end_offscreen_paint(&mut self) {
        self.session.backend_mut().end_offscreen_paint();
    }

    fn blit_offscreen(&mut self, handle: &crate::draw::ImageHandle, dst_rect: crate::core::Rect) {
        self.session.backend_mut().blit_offscreen(handle, dst_rect);
    }

    fn blit_offscreen_src(
        &mut self,
        handle: &crate::draw::ImageHandle,
        src_rect: crate::core::Rect,
        dst_rect: crate::core::Rect,
    ) {
        self.session
            .backend_mut()
            .blit_offscreen_src(handle, src_rect, dst_rect);
    }
}
