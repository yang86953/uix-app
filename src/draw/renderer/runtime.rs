//! 唯一图形运行时。
//!
//! [`Renderer`] 统一持有帧会话；CPU、GPU 以及 CPU 像素上传之间的
//! 差异只保留为后端与呈现策略，不再要求上层选择不同的引擎实现。

use std::time::Instant;

use crate::core::{Errc, Error, PresentDamageTracker, Rect};
use crate::draw::backend::{BackendKind, CpuBackend, DamageRegion};
// 引入唯一通用 GPU backend，避免经由单函数兼容 factory 转发。
use crate::draw::backend::gpu::GpuBackend;
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::renderer::RenderSession;
use crate::draw::renderer::{GraphicsFailure, RenderOutcome};
// test-harness 使用共享信号安排 owner-thread 帧回读。
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::{GraphicsFaultSignal, SurfaceReadbackRequest};
use crate::draw::{Canvas2D, GraphicsCapabilities, RasterPipeline, RenderTarget, UpdateStrategy};
// 引入 factory 已验证的正交 recipe owner。
use crate::platform::presentation::rhi::GraphicsRecipeOwner;
// 引入 PixelUpload presentation 的窄 owner 与 present 状态。
use crate::platform::presentation::rhi::{PixelUploadRecipeOwner, PresentTestResult};

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
    // 保存构造期已验证的 PixelUpload recipe owner。
    owner: PixelUploadRecipeOwner,
    damage_tracker: PresentDamageTracker,
    logical_width: i32,
    logical_height: i32,
}

impl PixelUploadPresentation {
    // 只接受已经通过构造期门禁的 PixelUpload owner。
    fn new(owner: PixelUploadRecipeOwner) -> Self {
        // 保存已验证 owner 与 presentation 私有状态。
        Self {
            // PixelUpload presentation 独占 native owner。
            owner,
            // 新 presentation 尚未提交任何 damage。
            damage_tracker: PresentDamageTracker::new(),
            // 首次同步前使用最小逻辑宽度。
            logical_width: 1,
            // 首次同步前使用最小逻辑高度。
            logical_height: 1,
        }
    }

    // 通过专用 PixelUpload surface 契约执行逻辑尺寸 resize。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 已验证 owner 统一处理视图丢失与逻辑尺寸归一化。
        self.owner.resize_surface(width, height)
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
    // 仅在显式测试 feature 下保存应用与当前 Renderer 共用的回读信号。
    #[cfg(feature = "test-harness")]
    test_graphics: Option<GraphicsFaultSignal>,
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

    /// 根据 native factory 已验证的正交 recipe owner 构造统一运行时。
    pub(crate) fn from_recipe_owner(owner: GraphicsRecipeOwner) -> Result<Self, Error> {
        // renderer 只分派稳定 owner 枚举，不再查询迁移期 context capability。
        match owner {
            // GPU recipe 已经完成 thin RHI 与 lifecycle 构造门禁。
            GraphicsRecipeOwner::Gpu(owner) => {
                // 所有具体 GraphicsApi 共用同一个薄 RHI GPU backend。
                let backend = GpuBackend::new_gpu_only(owner)?;
                // 将唯一 GPU backend 直接注入通用会话。
                Ok(Self::with_session(
                    // RenderSession 只接收已经完成 recipe 门禁的 backend。
                    RenderSession::with_backend(Box::new(backend)),
                    // GPU recipe 的最终呈现由 backend 管理。
                    Presentation::BackendManaged,
                ))
            }
            // PixelUpload recipe 已经完成 CPU raster 与专用 surface 构造门禁。
            GraphicsRecipeOwner::PixelUpload(owner) => {
                // presentation 只持有已验证 owner。
                let mut upload = PixelUploadPresentation::new(owner);
                // 创建 CPU raster session 承接待上传的 retained pixels。
                let session = match RenderSession::new(BackendKind::Cpu) {
                    // 保存可用的 CPU session。
                    Ok(session) => session,
                    // session 构造失败时关闭已经通过门禁的 native context。
                    Err(error) => {
                        return match upload.owner.try_shutdown() {
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
        }
    }

    /// 返回运行时持有的统一渲染会话。
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
            // 普通构造不隐式开放测试控制面，由应用组合根显式注入。
            #[cfg(feature = "test-harness")]
            test_graphics: None,
        }
    }

    // 将应用级测试信号绑定到当前 Renderer 帧边界。
    #[cfg(feature = "test-harness")]
    pub(crate) fn with_test_graphics_signal(mut self, signal: GraphicsFaultSignal) -> Self {
        // 先声明已有消费者，随后应用请求才能获得票据。
        signal.attach_surface_readback();
        // Renderer 保存同一信号并在 present 前消费请求。
        self.test_graphics = Some(signal);
        // 返回仍由当前 composition root 唯一拥有的 Renderer。
        self
    }

    // 在最终 present 前把一次应用回读请求下沉到当前 backend 事务。
    #[cfg(feature = "test-harness")]
    fn arm_surface_readback_for_test(
        // 借用当前唯一 Renderer owner。
        &mut self,
        // 返回请求和安排结果，最终 present 后再消费真正的像素结果。
    ) -> Option<(SurfaceReadbackRequest, crate::core::Result<()>)> {
        // 没有注入测试信号时不产生任何生产路径开销。
        let request = self.test_graphics.as_ref()?.take_surface_readback()?;
        // 只有 backend-managed GPU 能在最终 composite 内部提供准确时序。
        let result = if matches!(self.presentation.kind(), PresentationKind::BackendManaged) {
            // RenderSession 保持 owner-thread 门禁并安排下一次 composite 观察点。
            self.session.request_surface_readback_for_test()
        } else {
            // 外部 presenter 与 PixelUpload 不得冒充 GPU surface 验收。
            Err(Error::new(
                // 当前 recipe 不支持该测试能力。
                Errc::NotImplemented,
                // 明确限制到 backend-managed surface。
                "surface readback requires a backend-managed renderer",
            ))
        };
        // 延迟完成，最终 present 返回后才能消费像素或失败。
        Some((request, result))
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
        let caps = upload.owner.caps();
        let present_surface = upload.owner.present_surface();
        // PixelUpload recipe 只提交单一 CPU retained buffer，不持有 acquired image 身份。
        let present_image = None;
        let prepared_damage = upload.damage_tracker.prepare(
            caps.present_coherency,
            present_surface,
            present_image,
            present_damage,
        );
        let (damage_plan, damage_commit) = prepared_damage.into_parts();
        // 直接经已验证 PixelUpload owner 提交 CPU retained pixels。
        upload
            .owner
            .present_pixels(cpu.pixels(), width, height, damage_plan.present_damage)?;
        upload.damage_tracker.commit_prepared(damage_commit);
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
                let present_surface = upload.owner.present_surface();
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
            upload.owner.try_shutdown()?;
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
                let present_surface = upload.owner.present_surface();
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
        // test-harness 在 present 前安排由最终 composite 精确消费的回读请求。
        #[cfg(feature = "test-harness")]
        let pending_readback = self.arm_surface_readback_for_test();
        // 保存最终呈现结果，回读票据只接受同一帧的最终状态。
        let final_outcome = match self.presentation.kind() {
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
        };
        // test-harness 在最终呈现结果确定后完成一次性票据。
        #[cfg(feature = "test-harness")]
        if let Some((request, armed)) = pending_readback {
            // 安排成功时按最终呈现状态消费或清理 backend 事务。
            let completion = match (armed, &final_outcome) {
                // backend-managed present 成功后取得同一事务保存的规范像素。
                (Ok(()), RenderOutcome::Present(_)) => {
                    // 结果消费同时清理 backend 的单槽请求状态。
                    self.session.take_surface_readback_for_test()
                }
                // present 失败时先清理 backend 请求，再返回真正提交失败。
                (Ok(()), RenderOutcome::Failed(failure)) => {
                    // 忽略未到达或已产生的诊断结果，允许后续重新请求。
                    let _ = self.session.take_surface_readback_for_test();
                    // 最终提交失败优先成为该帧票据的原因。
                    Err(failure.error().clone())
                }
                // 成功安排却没有到达 backend-managed 最终状态属于内部不一致。
                (Ok(()), _) => {
                    // 清理 backend 单槽，避免一次异常状态永久阻塞后续测试。
                    let _ = self.session.take_surface_readback_for_test();
                    // 返回稳定的渲染状态错误。
                    Err(Error::new(
                        // 这是 Renderer 与 presentation recipe 的状态不一致。
                        Errc::InvalidState,
                        // 提供不依赖平台文本的稳定诊断。
                        "surface readback frame did not reach a backend-managed present",
                    ))
                }
                // 安排阶段的能力或状态错误无需触碰 backend 结果槽。
                (Err(error), _) => Err(error),
            };
            // 消费请求并向非 UI 测试线程交付结果。
            request.complete(completion);
        }
        // 返回原有渲染结果，不让测试观测改变生产状态机。
        final_outcome
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
        // PixelUpload 和外部 presenter 没有当前 GPU RHI 注入契约。
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
        // PixelUpload 和外部 presenter 没有当前 GPU RHI 注入契约。
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
                let dpr = upload.owner.present_surface().device_pixel_ratio;
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

    // Renderer 只转发检查式 Picture 创建，不建立第二个资源 owner。
    fn try_create_offscreen(
        // 借用当前 renderer 会话的唯一可变 owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 保留 backend 的正常无资源或 typed failure。
    ) -> Result<Option<ImageHandle>, Error> {
        // 资源创建仍由已选择 backend 唯一执行。
        self.session
            // 借用当前会话拥有的 backend。
            .backend_mut()
            // 透传检查式 Picture 创建结果。
            .try_create_offscreen(width, height)
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_destroy_offscreen(handle)
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.session.backend_mut().offscreen_canvas(handle)
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

    // 把公开能力查询转发到当前唯一 backend owner。
    fn supports_backdrop_blur(&self) -> bool {
        // PixelUpload/CPU backend 会通过默认实现明确返回 false。
        self.session.backend().supports_backdrop_blur()
    }

    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        self.session.backend_mut().snapshot_overlay_backdrop()
    }

    // 将 overlay backdrop blur 交给当前 backend 的唯一资源 owner。
    fn blur_overlay_backdrop(&mut self, region: Rect, radius: f32) -> Result<bool, Error> {
        // 保留 backend 的执行结果和 typed failure。
        self.session
            // 只借用当前 owner-thread backend。
            .backend_mut()
            // 不在 Renderer 门面复制多阶段效果语义。
            .blur_overlay_backdrop(region, radius)
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

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_begin_offscreen_paint(handle)
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.session.backend_mut().try_flush_offscreen_paint(handle)
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
