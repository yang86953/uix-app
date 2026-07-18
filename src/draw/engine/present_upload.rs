//! Engine-managed presentation through a native GPU context.
//!
//! This engine keeps the existing CPU Canvas2D raster path, then uploads the
//! frame pixels to a non-GL swapchain at present time.

use crate::core::{Error, PresentDamageTracker, Rect};
use crate::draw::backend::{BackendKind, CpuBackend, DamageRegion};
use crate::draw::engine::{GraphicsFailure, RenderOutcome};
use crate::draw::pipeline::RenderSession;
use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::primitives::color::Color;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::draw::ImageHandle;
use crate::native::traits::present::{
    IGraphicsContext, PresentDamage, PresentFrame, PresentTestResult,
};

pub struct PresentUploadEngine {
    pub(crate) session: RenderSession,
    gpu_ctx: Box<dyn IGraphicsContext>,
    pub clear_color: Color,
    present_damage_tracker: PresentDamageTracker,
    logical_width: i32,
    logical_height: i32,
    shutdown: bool,
}

impl PresentUploadEngine {
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let session = match RenderSession::new(BackendKind::Cpu) {
            Ok(session) => session,
            Err(err) => {
                let mut ctx = gpu_ctx;
                ctx.try_shutdown()?;
                return Err(err);
            }
        };
        Ok(Self {
            session,
            gpu_ctx,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            present_damage_tracker: PresentDamageTracker::new(),
            logical_width: 1,
            logical_height: 1,
            shutdown: false,
        })
    }

    pub fn backend_name(&self) -> &'static str {
        self.gpu_ctx.graphics_backend().as_str()
    }

    fn sync_clear_color(&mut self) {
        if let Some(cpu) = self.session.cpu_backend_mut() {
            cpu.set_clear_color(self.clear_color);
        }
    }

    fn cpu(&mut self) -> Option<&mut CpuBackend> {
        self.sync_clear_color();
        self.session.cpu_backend_mut()
    }
}

impl GraphicsEngine for PresentUploadEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // The native factory owns creation against the real surface. Calling
        // `IGraphicsContext::initialize` here would reinitialize a live
        // context with a null surface, so this stage only creates the CPU
        // draw session at the factory-reported drawable size.
        let actual_w = self.gpu_ctx.width().max(1);
        let actual_h = self.gpu_ctx.height().max(1);
        self.sync_clear_color();
        self.session.initialize(actual_w, actual_h)?;
        self.logical_width = width.max(1);
        self.logical_height = height.max(1);
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        self.session.try_shutdown()?;
        self.gpu_ctx.try_shutdown()?;
        self.shutdown = true;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        let logical_w = width.max(1);
        let logical_h = height.max(1);
        self.gpu_ctx.resize(logical_w, logical_h)?;
        // 与 NativeGpuBackend 一致：CPU 表面跟 GPU 实际客户区对齐。
        let actual_w = self.gpu_ctx.width().max(1);
        let actual_h = self.gpu_ctx.height().max(1);
        self.sync_clear_color();
        self.session.resize(actual_w, actual_h)?;
        self.logical_width = logical_w;
        self.logical_height = logical_h;
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.sync_clear_color();
        // CPU canvas is retained across frames; honor DirtyRects so undamaged
        // pixels survive and paint/present can stay damage-scoped.
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if matches!(outcome, RenderOutcome::Failed(_)) {
            return outcome;
        }
        if let Some(cpu) = self.session.cpu_backend() {
            let width = cpu.width();
            let height = cpu.height();
            let caps = self.gpu_ctx.caps();
            let present_surface = self.gpu_ctx.present_surface();
            let present_image = self.gpu_ctx.present_image();
            let damage = self
                .present_damage_tracker
                .plan(
                    caps.present_coherency,
                    present_surface,
                    present_image,
                    present_damage,
                )
                .present_damage;
            let damage_full = matches!(damage, PresentDamage::Full);
            let pixels = (width as u64).saturating_mul(height as u64);
            if crate::core::perf_probe::skip_present_enabled() {
                crate::core::perf_probe::record_present(
                    crate::core::perf_probe::PresentProbeSample {
                        present_us: 0,
                        upload_copy_us: 0,
                        fence_wait_us: 0,
                        submit_present_us: 0,
                        pixels,
                        damage_full: if damage_full { 1 } else { 0 },
                        skipped: 1,
                    },
                );
                crate::core::log::info_fn(format!(
                    "present_upload_us=0 pixels={} damage_full={} backend={} skipped=1",
                    pixels,
                    if damage_full { 1 } else { 0 },
                    self.backend_name(),
                ));
            } else {
                let frame = PresentFrame::PixelBuffer {
                    pixels: cpu.pixels(),
                    width,
                    height,
                    // CPU upload has the same preservation precondition as a
                    // swapchain present. Do not let a caller turn an unproven
                    // partial-present context into a partial redraw path.
                    damage,
                };
                let present_t0 = std::time::Instant::now();
                let present_result = self.gpu_ctx.present(&frame);
                let present_us = present_t0.elapsed().as_micros();
                let mut sample = crate::core::perf_probe::take_present();
                sample.present_us = present_us;
                sample.pixels = pixels;
                sample.damage_full = if damage_full { 1 } else { 0 };
                sample.skipped = 0;
                crate::core::perf_probe::record_present(sample);
                crate::core::log::info_fn(format!(
                    "present_upload_us={} upload_copy_us={} fence_wait_us={} submit_present_us={} pixels={} damage_full={} backend={}",
                    present_us,
                    sample.upload_copy_us,
                    sample.fence_wait_us,
                    sample.submit_present_us,
                    pixels,
                    if damage_full { 1 } else { 0 },
                    self.backend_name(),
                ));
                if let Err(err) = present_result {
                    return RenderOutcome::Failed(GraphicsFailure::from_error(err));
                }
                self.present_damage_tracker.commit(
                    caps.present_coherency,
                    present_surface,
                    present_image,
                    present_damage,
                );
            }
        }
        outcome
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        self.gpu_ctx.test_present()
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        (self.logical_width, self.logical_height)
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        // CPU canvas 跨帧保留，可安全执行局部清绘与重叠滚动复制；最终提交仍由
        // context 的 present coherency 独立门控，Vulkan FullOnly 不会被冒充为窄提交。
        GraphicsCapabilities::engine_managed_retained_pixels()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.gpu_ctx.device_pixel_ratio()
    }

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        self.session.backend_mut().create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        self.session.backend_mut().destroy_offscreen(handle);
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_destroy_offscreen(handle)
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        if let Some(cpu) = self.cpu() {
            cpu.blit_offscreen(handle, dst_rect);
        }
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        if let Some(cpu) = self.cpu() {
            cpu.blit_offscreen_src(handle, src_rect, dst_rect);
        }
    }

    fn blit_offscreen_to_canvas(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn Canvas2D,
    ) {
        if let Some(cpu) = self.session.cpu_backend() {
            cpu.blit_offscreen_to_canvas(handle, src_rect, dst_rect, canvas);
        }
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.session.cpu_backend()?.copy_offscreen_pixels(handle)
    }

    fn copy_frame_pixels(&self) -> Option<(Vec<u32>, i32)> {
        let cpu = self.session.cpu_backend()?;
        Some((cpu.pixels().to_vec(), cpu.width()))
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        self.session
            .backend_mut()
            .try_execute_encoded_picture(handle, encoder)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        self.session
            .backend_mut()
            .try_execute_encoded_frame(encoder)
    }

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        self.session.backend_mut().begin_offscreen_paint(handle)
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        self.session.backend_mut().flush_offscreen_paint(handle);
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_flush_offscreen_paint(handle)
    }

    fn end_offscreen_paint(&mut self) {
        self.session.backend_mut().end_offscreen_paint();
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.session.backend_mut().try_end_offscreen_paint()
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        self.session
            .backend_mut()
            .try_blit_offscreen_src(handle, src_rect, dst_rect)
    }

    fn memory_usage(&self) -> usize {
        self.session
            .cpu_backend()
            .map(|cpu| cpu.memory_usage())
            .unwrap_or(0)
    }
}

impl Drop for PresentUploadEngine {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format!(
                "PresentUploadEngine {} checked shutdown failed: {}",
                self.backend_name(),
                error.short_what()
            ));
        }
    }
}
