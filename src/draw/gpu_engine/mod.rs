// ============================================================================
// draw/gpu_engine/mod.rs — GPU 渲染引擎（GLES 3.0）
// ============================================================================

use crate::core::Error;
use crate::draw::backend::registry::create_native_raster_backend;
use crate::draw::backend::DamageRegion;
use crate::draw::engine::{GraphicsFailure, RenderOutcome};
use crate::draw::pipeline::{
    EncodedFrameExecution, EncodedPictureExecution, FrameEncoder, RenderSession,
};
use crate::draw::traits::{
    Canvas2D, GraphicsCapabilities, GraphicsEngine, RasterPipeline, UpdateStrategy,
};
use crate::native::traits::present::{IGraphicsContext, PresentTestResult};
use std::time::Instant;

/// GPU 渲染引擎 — 委托 `RenderSession` + `RenderBackend`（GL / D3D11 / …）。
pub struct GpuEngine {
    session: RenderSession,
}

impl GpuEngine {
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match create_native_raster_backend(gpu_ctx) {
            Ok(backend) => RenderSession::with_backend(backend),
            Err(err) => return Err(err),
        };
        Ok(Self { session })
    }

    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    pub fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }

    fn make_current(&mut self) -> Result<(), Error> {
        self.session.backend_mut().make_current()
    }
}

impl GraphicsEngine for GpuEngine {
    fn initialize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.initialize_prepared(w, h)?;
        self.make_current()?;
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.session.try_shutdown()
    }

    fn resize(&mut self, w: i32, h: i32) -> Result<(), Error> {
        self.session.resize(w, h)?;
        self.make_current()?;
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        if let Err(error) = self.make_current() {
            return RenderOutcome::Failed(GraphicsFailure::from_error(error));
        }
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if !matches!(outcome, RenderOutcome::Present(_)) {
            return outcome;
        }
        if let Err(error) = self.session.backend_mut().present(present_damage) {
            return RenderOutcome::Failed(GraphicsFailure::from_error(error));
        }
        outcome
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        self.session.backend_mut().test_present()
    }

    fn note_presented_at(&mut self, now: Instant) {
        if let Some(backend) = self.session.native_gpu_backend_mut() {
            backend.note_presented_at(now);
        }
    }

    fn idle_resource_deadline(&self) -> Option<Instant> {
        self.session
            .native_gpu_backend()
            .and_then(|backend| backend.idle_resource_deadline())
    }

    fn release_idle_resources(&mut self, now: Instant) {
        if let Some(backend) = self.session.native_gpu_backend_mut() {
            backend.release_idle_resources(now);
        }
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.session.graphics_capabilities()
    }

    fn raster_pipeline(&self) -> RasterPipeline {
        RasterPipeline::GpuNative
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

    fn try_destroy_offscreen(&mut self, handle: crate::draw::ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_destroy_offscreen(handle)
    }

    fn offscreen_canvas(&mut self, handle: &crate::draw::ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn copy_offscreen_pixels(&self, handle: &crate::draw::ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.session.backend().copy_offscreen_pixels(handle)
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &crate::draw::ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        encoder.validate_gpu_native().map_err(|error| {
            Error::new(
                crate::core::Errc::InvalidState,
                format!("GpuEngine rejected non-native Picture: {error}"),
            )
        })?;
        self.session
            .backend_mut()
            .try_execute_encoded_picture(handle, encoder)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        encoder.validate_gpu_native().map_err(|error| {
            Error::new(
                crate::core::Errc::InvalidState,
                format!("GpuEngine rejected non-native frame: {error}"),
            )
        })?;
        self.session
            .backend_mut()
            .try_execute_encoded_frame(encoder)
    }

    fn begin_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) -> bool {
        self.session.backend_mut().begin_offscreen_paint(handle)
    }

    fn try_begin_offscreen_paint(
        &mut self,
        handle: &crate::draw::ImageHandle,
    ) -> Result<(), Error> {
        self.session.backend_mut().try_begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &crate::draw::ImageHandle) {
        self.session.backend_mut().flush_offscreen_paint(handle);
    }

    fn try_flush_offscreen_paint(
        &mut self,
        handle: &crate::draw::ImageHandle,
    ) -> Result<(), Error> {
        self.session.backend_mut().try_flush_offscreen_paint(handle)
    }

    fn end_offscreen_paint(&mut self) {
        self.session.backend_mut().end_offscreen_paint();
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.session.backend_mut().try_end_offscreen_paint()
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

    fn try_blit_offscreen_src(
        &mut self,
        handle: &crate::draw::ImageHandle,
        src_rect: crate::core::Rect,
        dst_rect: crate::core::Rect,
    ) -> Result<(), Error> {
        self.session
            .backend_mut()
            .try_blit_offscreen_src(handle, src_rect, dst_rect)
    }
}
