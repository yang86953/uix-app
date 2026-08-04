//! 唯一图形运行时。
//!
//! [`Renderer`] 统一持有帧会话；CPU、GPU 以及 CPU 像素上传之间的
//! 差异只保留为后端与呈现策略，不再要求上层选择不同的引擎实现。

use std::time::Instant;

use crate::core::{Errc, Error, PresentDamageTracker, Rect};
use crate::draw::backend::factory::create_native_raster_backend;
use crate::draw::backend::{BackendKind, CpuBackend, DamageRegion};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::renderer::RenderSession;
use crate::draw::renderer::{GraphicsFailure, RenderOutcome};
use crate::draw::{Canvas2D, GraphicsCapabilities, RasterPipeline, RenderTarget, UpdateStrategy};
use crate::native::present::{
    IGraphicsContext, PresentDamage, PresentFrame, PresentMode, PresentTestResult, RasterMode,
};

/// 最终呈现由谁完成。
enum Presentation {
    /// CPU retained pixels are handed to the platform's sole `IPresenter`.
    External,
    /// The selected render backend owns swapchain submission and presentation.
    BackendManaged,
    /// CPU rasterization followed by upload through an already prepared context.
    PixelUpload(PixelUploadPresentation),
}

#[derive(Clone, Copy)]
enum PresentationKind {
    External,
    BackendManaged,
    PixelUpload,
}

impl Presentation {
    fn kind(&self) -> PresentationKind {
        match self {
            Self::External => PresentationKind::External,
            Self::BackendManaged => PresentationKind::BackendManaged,
            Self::PixelUpload(_) => PresentationKind::PixelUpload,
        }
    }
}

struct PixelUploadPresentation {
    context: Box<dyn IGraphicsContext>,
    damage_tracker: PresentDamageTracker,
    logical_width: i32,
    logical_height: i32,
}

impl PixelUploadPresentation {
    fn new(context: Box<dyn IGraphicsContext>) -> Self {
        Self {
            context,
            damage_tracker: PresentDamageTracker::new(),
            logical_width: 1,
            logical_height: 1,
        }
    }

    fn sync_logical_extent(&mut self) {
        let dpr = self.context.caps().device_pixel_ratio;
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            dpr
        } else {
            1.0
        };
        let logical = |physical: i32| ((physical.max(1) as f32 / dpr).round() as i32).max(1);
        self.logical_width = logical(self.context.width());
        self.logical_height = logical(self.context.height());
    }
}

/// UIX 唯一的图形运行时类型。
///
/// 绘制、离屏资源和帧生命周期始终通过同一个 [`Renderer`]；CPU/GPU 只改变
/// `RenderSession` 内的后端以及最终呈现策略。
pub struct Renderer {
    session: RenderSession,
    presentation: Presentation,
    /// 脏区清除时使用的背景色；仅 CPU 光栅后端消费。
    pub clear_color: Color,
    shutdown: bool,
}

impl Renderer {
    /// 创建 CPU 光栅、平台外部呈现的运行时。
    pub fn cpu() -> Self {
        let session = match RenderSession::new(BackendKind::Cpu) {
            Ok(session) => session,
            Err(error) => {
                tracing::error!("RenderSession 创建失败: {}", error.short_what());
                RenderSession::with_backend(Box::new(CpuBackend::new()))
            }
        };
        Self::with_session(session, Presentation::External)
    }

    /// 根据 native context 的正交 raster/present 能力构造统一运行时。
    pub(crate) fn from_context(mut context: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        let caps = context.caps();
        match (caps.raster, caps.present) {
            (RasterMode::GpuNative, PresentMode::Swapchain) => {
                let backend = create_native_raster_backend(context)?;
                Ok(Self::with_session(
                    RenderSession::with_backend(backend),
                    Presentation::BackendManaged,
                ))
            }
            (RasterMode::Cpu, PresentMode::PixelUpload) => {
                let session = match RenderSession::new(BackendKind::Cpu) {
                    Ok(session) => session,
                    Err(error) => {
                        return match context.try_shutdown() {
                            Ok(()) => Err(error),
                            Err(cleanup_error) => Err(cleanup_error.with_source(error)),
                        };
                    }
                };
                Ok(Self::with_session(
                    session,
                    Presentation::PixelUpload(PixelUploadPresentation::new(context)),
                ))
            }
            (raster, present) => {
                let error = Error::new(
                    Errc::InvalidArgument,
                    format!(
                        "Renderer::from_context: unsupported RasterMode × PresentMode combination: \
                         {raster} × {present}"
                    ),
                );
                match context.try_shutdown() {
                    Ok(()) => Err(error),
                    Err(cleanup_error) => Err(cleanup_error.with_source(error)),
                }
            }
        }
    }

    pub fn session(&self) -> &RenderSession {
        &self.session
    }

    #[cfg(test)]
    pub(crate) fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }

    fn with_session(session: RenderSession, presentation: Presentation) -> Self {
        Self {
            session,
            presentation,
            clear_color: Color::from_rgba(0, 0, 0, 0),
            shutdown: false,
        }
    }

    fn sync_clear_color(&mut self) {
        if let Some(cpu) = self.session.cpu_backend_mut() {
            cpu.set_clear_color(self.clear_color);
        }
    }

    fn make_backend_current(&mut self) -> Result<(), Error> {
        self.session.backend_mut().make_current()
    }

    fn present_uploaded_pixels(&mut self, present_damage: &DamageRegion) -> Result<(), Error> {
        let Presentation::PixelUpload(upload) = &mut self.presentation else {
            return Err(Error::new(
                Errc::InvalidState,
                "pixel upload requested without a PixelUpload presentation",
            ));
        };
        let cpu = self.session.cpu_backend().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "PixelUpload presentation requires the CPU render backend",
            )
        })?;
        let width = cpu.width();
        let height = cpu.height();
        let caps = upload.context.caps();
        let present_surface = upload.context.present_surface();
        let present_image = upload.context.present_image();
        let damage = upload
            .damage_tracker
            .plan(
                caps.present_coherency,
                present_surface,
                present_image,
                present_damage,
            )
            .present_damage;
        let damage_full = matches!(damage, PresentDamage::Full);
        let pixels = (width as u64).saturating_mul(height as u64);
        let backend_name = upload.context.graphics_backend().as_str();

        if crate::core::perf_probe::skip_present_enabled() {
            crate::core::perf_probe::record_present(crate::core::perf_probe::PresentProbeSample {
                present_us: 0,
                upload_copy_us: 0,
                fence_wait_us: 0,
                submit_present_us: 0,
                surface_present_cpu_us: 0,
                pixels,
                drawable_pixels: 0,
                drawable_width: 0,
                drawable_height: 0,
                damage_full: u8::from(damage_full),
                skipped: 1,
            });
            tracing::info!(
                "present_upload_us=0 pixels={} damage_full={} backend={} skipped=1",
                pixels,
                u8::from(damage_full),
                backend_name,
            );
            return Ok(());
        }

        let frame = PresentFrame::PixelBuffer {
            pixels: cpu.pixels(),
            width,
            height,
            damage,
        };
        let present_t0 = Instant::now();
        let present_result = upload.context.present(&frame);
        let present_us = present_t0.elapsed().as_micros();
        let mut sample = crate::core::perf_probe::take_present();
        sample.present_us = present_us;
        sample.pixels = pixels;
        sample.damage_full = u8::from(damage_full);
        sample.skipped = 0;
        crate::core::perf_probe::record_present(sample);
        tracing::info!(
            "present_upload_us={} upload_copy_us={} fence_wait_us={} submit_present_us={} \
             pixels={} damage_full={} backend={}",
            present_us,
            sample.upload_copy_us,
            sample.fence_wait_us,
            sample.submit_present_us,
            pixels,
            u8::from(damage_full),
            backend_name,
        );
        present_result?;
        upload.damage_tracker.commit(
            caps.present_coherency,
            present_surface,
            present_image,
            present_damage,
        );
        Ok(())
    }

    fn validate_native_picture(&self, encoder: &FrameEncoder) -> Result<(), Error> {
        if !matches!(self.presentation.kind(), PresentationKind::BackendManaged) {
            return Ok(());
        }
        encoder.validate_gpu_native().map_err(|error| {
            Error::new(
                Errc::InvalidState,
                format!("Renderer rejected non-native Picture: {error}"),
            )
        })
    }

    fn validate_native_frame(&self, encoder: &FrameEncoder) -> Result<(), Error> {
        if !matches!(self.presentation.kind(), PresentationKind::BackendManaged) {
            return Ok(());
        }
        encoder.validate_gpu_native().map_err(|error| {
            Error::new(
                Errc::InvalidState,
                format!("Renderer rejected non-native frame: {error}"),
            )
        })
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::cpu()
    }
}

impl RenderTarget for Renderer {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.shutdown = false;
        self.sync_clear_color();
        match &mut self.presentation {
            Presentation::External => self.session.initialize(width, height),
            Presentation::BackendManaged => {
                self.session.initialize_prepared(width, height)?;
                self.session.backend_mut().make_current()
            }
            Presentation::PixelUpload(upload) => {
                let actual_width = upload.context.width().max(1);
                let actual_height = upload.context.height().max(1);
                self.session.initialize(actual_width, actual_height)?;
                upload.sync_logical_extent();
                Ok(())
            }
        }
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        self.session.try_shutdown()?;
        if let Presentation::PixelUpload(upload) = &mut self.presentation {
            upload.context.try_shutdown()?;
        }
        self.shutdown = true;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.sync_clear_color();
        match &mut self.presentation {
            Presentation::External => self.session.resize(width, height),
            Presentation::BackendManaged => {
                self.session.resize(width, height)?;
                self.session.backend_mut().make_current()
            }
            Presentation::PixelUpload(upload) => {
                upload.context.resize(width.max(1), height.max(1))?;
                let actual_width = upload.context.width().max(1);
                let actual_height = upload.context.height().max(1);
                self.session.resize(actual_width, actual_height)?;
                upload.sync_logical_extent();
                Ok(())
            }
        }
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.sync_clear_color();
        if matches!(self.presentation.kind(), PresentationKind::BackendManaged) {
            if let Err(error) = self.make_backend_current() {
                return RenderOutcome::Failed(GraphicsFailure::from_error(error));
            }
        }
        self.session.begin_frame(strategy)
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.session.end_frame();
        if !matches!(outcome, RenderOutcome::Present(_)) {
            return outcome;
        }
        match self.presentation.kind() {
            PresentationKind::External => match outcome {
                RenderOutcome::Present(damage) => RenderOutcome::PresentPending(damage),
                other => other,
            },
            PresentationKind::BackendManaged => {
                if let Err(error) = self.session.backend_mut().present(present_damage) {
                    RenderOutcome::Failed(GraphicsFailure::from_error(error))
                } else {
                    outcome
                }
            }
            PresentationKind::PixelUpload => match self.present_uploaded_pixels(present_damage) {
                Ok(()) => outcome,
                Err(error) => RenderOutcome::Failed(GraphicsFailure::from_error(error)),
            },
        }
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        match &mut self.presentation {
            Presentation::External => Err(Error::new(
                Errc::NotImplemented,
                "external-presenter Renderer does not support idle present tests",
            )),
            Presentation::BackendManaged => self.session.backend_mut().test_present(),
            Presentation::PixelUpload(upload) => upload.context.test_present(),
        }
    }

    // 将测试故障安排到 backend-managed GPU 的真实 RHI 设备边界。
    #[cfg(feature = "test-harness")]
    fn inject_graphics_device_lost_for_test(&mut self) -> Result<(), Error> {
        // PixelUpload 和外部 presenter 没有当前 D3D11 RHI 注入契约。
        if !matches!(self.presentation.kind(), PresentationKind::BackendManaged) {
            return Err(Error::new(
                Errc::NotImplemented,
                "graphics device-lost injection requires a backend-managed GPU",
            ));
        }
        // RenderSession 保持 owner-thread 检查并继续下沉到 GpuBackend。
        self.session.inject_graphics_device_lost_for_test()
    }

    // 将测试 surface-lost 安排到 backend-managed GPU 的真实 RHI surface 边界。
    #[cfg(feature = "test-harness")]
    fn inject_graphics_surface_lost_for_test(&mut self) -> Result<(), Error> {
        // PixelUpload 和外部 presenter 没有当前 D3D11 RHI 注入契约。
        if !matches!(self.presentation.kind(), PresentationKind::BackendManaged) {
            return Err(Error::new(
                Errc::NotImplemented,
                "surface-lost injection requires a backend-managed GPU",
            ));
        }
        // RenderSession 保持 owner-thread 检查并继续下沉到 GpuBackend。
        self.session.inject_graphics_surface_lost_for_test()
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.session.canvas_2d()
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        match &self.presentation {
            Presentation::PixelUpload(upload) => (upload.logical_width, upload.logical_height),
            Presentation::External => {
                let canvas = self.session.canvas_2d();
                (canvas.width(), canvas.height())
            }
            Presentation::BackendManaged => (self.session.width(), self.session.height()),
        }
    }

    fn note_presented_at(&mut self, now: Instant) {
        if let Some(backend) = self.session.gpu_backend_mut() {
            backend.note_presented_at(now);
        }
    }

    fn idle_resource_deadline(&self) -> Option<Instant> {
        self.session
            .gpu_backend()
            .and_then(|backend| backend.idle_resource_deadline())
    }

    fn release_idle_resources(&mut self, now: Instant) {
        if let Some(backend) = self.session.gpu_backend_mut() {
            backend.release_idle_resources(now);
        }
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        match &self.presentation {
            Presentation::PixelUpload(_) => GraphicsCapabilities::backend_managed_retained_pixels(),
            Presentation::External | Presentation::BackendManaged => {
                self.session.graphics_capabilities()
            }
        }
    }

    fn raster_pipeline(&self) -> RasterPipeline {
        match &self.presentation {
            Presentation::BackendManaged => RasterPipeline::GpuNative,
            Presentation::External | Presentation::PixelUpload(_) => RasterPipeline::Cpu,
        }
    }

    fn device_pixel_ratio(&self) -> f32 {
        match &self.presentation {
            Presentation::PixelUpload(upload) => upload.context.device_pixel_ratio(),
            Presentation::External | Presentation::BackendManaged => {
                self.session.backend().device_pixel_ratio()
            }
        }
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
        self.session.backend_mut().blit_offscreen(handle, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        self.session
            .backend_mut()
            .blit_offscreen_src(handle, src_rect, dst_rect);
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
        self.session.backend().copy_offscreen_pixels(handle)
    }

    fn copy_frame_pixels(&self) -> Option<(Vec<u32>, i32)> {
        let cpu = self.session.cpu_backend()?;
        Some((cpu.pixels().to_vec(), cpu.width()))
    }

    fn snapshot_overlay_backdrop(&mut self) -> bool {
        self.session.backend_mut().snapshot_overlay_backdrop()
    }

    fn restore_overlay_backdrop(&mut self) -> bool {
        self.session.backend_mut().restore_overlay_backdrop()
    }

    fn release_overlay_backdrop(&mut self) {
        self.session.backend_mut().release_overlay_backdrop();
    }

    fn has_overlay_backdrop(&self) -> bool {
        self.session.backend().has_overlay_backdrop()
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        self.validate_native_picture(encoder)?;
        self.session
            .backend_mut()
            .try_execute_encoded_picture(handle, encoder)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        self.validate_native_frame(encoder)?;
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

    fn try_blur_offscreen(
        &mut self,
        handle: &ImageHandle,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
        self.session
            .backend_mut()
            .try_blur_offscreen(handle, region, radius)
    }

    fn memory_usage(&self) -> usize {
        self.session
            .cpu_backend()
            .map(CpuBackend::memory_usage)
            .unwrap_or(0)
    }

    fn diagnose_memory(&self) {
        tracing::info!("Renderer memory: {} bytes", self.memory_usage());
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            tracing::error!("Renderer checked shutdown failed: {}", error.short_what());
        }
    }
}
