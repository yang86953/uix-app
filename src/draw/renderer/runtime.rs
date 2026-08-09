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
use crate::native::present::{IGraphicsContext, PresentMode, PresentTestResult, RasterMode};

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
    // 只接受显式提供 PixelUpload surface 生命周期的 context。
    fn try_new(mut context: Box<dyn IGraphicsContext>) -> Result<Self, Error> {
        // recipe capability 与专用 surface 契约必须同时存在。
        if context.pixel_upload_surface().is_none() {
            // 构造不变量缺失时返回稳定 typed error。
            let error = Error::new(
                // 使用参数错误标记 factory 交付了不完整 recipe。
                Errc::InvalidArgument,
                // 明确指出缺失的专用执行边界。
                "CPU PixelUpload recipe requires a dedicated resize surface",
            );
            // 失败构造仍必须在 owner thread 检查式关闭 native context。
            return match context.try_shutdown() {
                // shutdown 成功时保留原始 recipe 错误。
                Ok(()) => Err(error),
                // shutdown 失败时保留清理错误并链接原始原因。
                Err(cleanup_error) => Err(cleanup_error.with_source(error)),
            };
        }
        // 保存已经通过 recipe 门禁的 context。
        Ok(Self {
            // PixelUpload presentation 独占 native context。
            context,
            // 新 presentation 尚未提交任何 damage。
            damage_tracker: PresentDamageTracker::new(),
            // 首次同步前使用最小逻辑宽度。
            logical_width: 1,
            // 首次同步前使用最小逻辑高度。
            logical_height: 1,
        })
    }

    // 通过专用 PixelUpload surface 契约执行逻辑尺寸 resize。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 构造后能力消失属于 native context 状态破坏。
        let Some(surface) = self.context.pixel_upload_surface() else {
            // 返回 typed 状态错误，禁止回退旧 IGraphicsContext resize。
            return Err(Error::new(
                // 使用 InvalidState 进入既有恢复路径。
                Errc::InvalidState,
                // 明确指出专用 surface 在生命周期中丢失。
                "PixelUpload presentation lost its dedicated resize surface",
            ));
        };
        // 归一化逻辑尺寸后交给专用 native surface。
        surface.resize_pixel_upload_surface(width.max(1), height.max(1))
    }

    // 从调用方已经读取的单一 surface 快照同步逻辑尺寸。
    fn sync_logical_extent(&mut self, surface: crate::core::PresentSurface) {
        // 只接受可稳定映射逻辑坐标的有限正 DPR。
        let dpr = surface.device_pixel_ratio;
        // 无效 native 元数据保守退回 identity 比例。
        let dpr = if dpr.is_finite() && dpr > 0.0 {
            // 有效比例原样用于逻辑换算。
            dpr
        } else {
            // 使用安全的 identity 映射。
            1.0
        };
        // 把同一快照内的物理尺寸换算为逻辑尺寸。
        let logical = |physical: i32| ((physical.max(1) as f32 / dpr).round() as i32).max(1);
        // 同步逻辑宽度。
        self.logical_width = logical(surface.drawable_width);
        // 同步逻辑高度。
        self.logical_height = logical(surface.drawable_height);
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
                // 在创建 CPU session 前验证 native PixelUpload surface 契约。
                let mut upload = PixelUploadPresentation::try_new(context)?;
                // 创建 CPU raster session 承接待上传的 retained pixels。
                let session = match RenderSession::new(BackendKind::Cpu) {
                    // 保存可用的 CPU session。
                    Ok(session) => session,
                    // session 构造失败时关闭已经通过门禁的 native context。
                    Err(error) => {
                        return match upload.context.try_shutdown() {
                            // native shutdown 成功时返回 CPU session 错误。
                            Ok(()) => Err(error),
                            // native shutdown 失败时链接 CPU session 错误。
                            Err(cleanup_error) => Err(cleanup_error.with_source(error)),
                        };
                    }
                };
                // 组合唯一 CPU session 与已经验证的 PixelUpload presentation。
                Ok(Self::with_session(
                    // CPU raster 继续由统一 RenderSession 持有。
                    session,
                    // 专用 presentation 持有 native PixelUpload surface。
                    Presentation::PixelUpload(upload),
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

    // 测试目标保留渲染 session 可变观测入口，供 renderer 契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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
        // PixelUpload recipe 只提交单一 CPU retained buffer，不持有 acquired image 身份。
        let present_image = None;
        let damage = upload
            .damage_tracker
            .plan(
                caps.present_coherency,
                present_surface,
                present_image,
                present_damage,
            )
            .present_damage;
        // 构造后专用提交视图消失属于 native context 状态破坏。
        let Some(surface) = upload.context.pixel_upload_surface() else {
            // 返回 typed 状态错误，禁止回退已移除的统一 present。
            return Err(Error::new(
                // 使用 InvalidState 进入既有恢复路径。
                Errc::InvalidState,
                // 明确指出专用 PixelUpload 提交边界缺失。
                "PixelUpload presentation lost its dedicated presentation surface",
            ));
        };
        // 直接经 PixelUpload recipe 的专用 surface 提交 CPU retained pixels。
        surface.present_pixels(cpu.pixels(), width, height, damage)?;
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
            // 原生 GPU context 已由 factory 构造完成；首帧准备统一在 session.begin_frame 执行。
            Presentation::BackendManaged => self.session.initialize_prepared(width, height),
            Presentation::PixelUpload(upload) => {
                // 一次读取初始化后的完整 PixelUpload surface 快照。
                let present_surface = upload.context.present_surface();
                // 从同一快照读取实际物理宽度。
                let actual_width = present_surface.drawable_width.max(1);
                // 从同一快照读取实际物理高度。
                let actual_height = present_surface.drawable_height.max(1);
                self.session.initialize(actual_width, actual_height)?;
                // 复用同一快照同步逻辑 extent。
                upload.sync_logical_extent(present_surface);
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
            // resize 只更新 surface；后续帧由 prepare_frame 恢复 owner-context 状态。
            Presentation::BackendManaged => self.session.resize(width, height),
            Presentation::PixelUpload(upload) => {
                // CPU PixelUpload 只通过专用 surface 契约重建 native drawable。
                upload.resize_surface(width, height)?;
                // resize 成功后一次读取完整 PixelUpload surface 快照。
                let present_surface = upload.context.present_surface();
                // 从同一快照读取 adapter 的实际物理宽度。
                let actual_width = present_surface.drawable_width.max(1);
                // 从同一快照读取 adapter 的实际物理高度。
                let actual_height = present_surface.drawable_height.max(1);
                // 让 CPU retained surface 与 native drawable 像素尺寸保持一致。
                self.session.resize(actual_width, actual_height)?;
                // 更新上层窗口使用的逻辑尺寸缓存。
                upload.sync_logical_extent(present_surface);
                // 返回两侧 surface 已同步的成功结果。
                Ok(())
            }
        }
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.sync_clear_color();
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
            // PixelUpload 没有 swapchain idle 状态，不借用 context 兼容入口。
            Presentation::PixelUpload(_) => Err(Error::new(
                // 保持与外部 presenter 相同的明确未支持分类。
                Errc::NotImplemented,
                // 把不适用原因绑定到当前 presentation recipe。
                "pixel-upload Renderer does not support idle present tests",
            )),
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
            Presentation::PixelUpload(upload) => {
                // PixelUpload 只从完整 live surface 快照读取 DPR。
                let dpr = upload.context.present_surface().device_pixel_ratio;
                // 无效 native 元数据保守回退 identity 比例。
                if dpr.is_finite() && dpr > 0.0 {
                    // 返回当前有效 DPR。
                    dpr
                } else {
                    // 保持 renderer 对非法元数据的既有安全语义。
                    1.0
                }
            }
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

    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        self.session.backend_mut().snapshot_overlay_backdrop()
    }

    fn restore_overlay_backdrop(&mut self) -> Result<bool, Error> {
        self.session.backend_mut().restore_overlay_backdrop()
    }

    fn release_overlay_backdrop(&mut self) -> Result<(), Error> {
        self.session.backend_mut().release_overlay_backdrop()
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
