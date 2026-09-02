//! Frame-boundary graphics recovery wrapper.
//!
//! The wrapped engine owns a live, thread-affine graphics context.  This
//! wrapper never probes or replaces it while a frame is being recorded: a
//! typed failure is returned first so the caller retains dirty state, then the
//! next `begin_frame` runs exactly one bounded recovery action.

use crate::core::{Error, Rect};
use crate::draw::backend::DamageRegion;
use crate::draw::geometry::types::ImageHandle;
use crate::draw::painting::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::draw::renderer::{
    GraphicsFailure, GraphicsRecovery, GraphicsRecoveryAction, RenderOutcome,
};
use crate::draw::{Canvas2D, GraphicsCapabilities, RenderTarget, UpdateStrategy};
use crate::platform::presentation::rhi::PresentTestResult;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

// 连续三次 probe/Present 矛盾才判定 surface 不再兼容，避免瞬时遮挡竞态触发重建。
const MAX_PRESENT_PROBE_CONFLICTS: u8 = 3;

/// One app-owned rebuild request shared by a Diagnostics recovery registration
/// and the owning [`RecoveryDriver`].
///
/// The request is intentionally narrow: it only says "the domain observed a
/// typed failure that needs the bounded graphics recovery sequence at the next
/// frame boundary". The sequence itself stays in [`GraphicsRecovery`] and the
/// rebuilder closure; this handle never probes or replaces an engine.
#[derive(Clone, Default)]
pub(crate) struct RebuildRequest {
    requested: Arc<AtomicBool>,
}

impl RebuildRequest {
    /// Records one rebuild request. Calling it from any thread is safe; the
    /// owning [`RecoveryDriver`] consumes it at its next frame boundary.
    pub(crate) fn request_rebuild(&self) {
        self.requested.store(true, Ordering::Release);
    }

    /// Read-only check whether a rebuild is currently requested.
    // 该读取入口只服务同模块单元测试，生产路径通过 take 消费请求。
    #[cfg(test)]
    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }

    fn take(&self) -> bool {
        self.requested.swap(false, Ordering::AcqRel)
    }
}

/// Recreates one initialized engine for a permitted recovery action.
///
/// The app-side closure owns the native surface and recipe cursor.  It must
/// only recreate the exact action requested here; this keeps probe policy out
/// of frame recording and makes fallback order auditable.
pub type RenderTargetRebuilder =
    Box<dyn FnMut(GraphicsRecoveryAction, i32, i32) -> Result<Box<dyn RenderTarget>, Error>>;

/// A render target plus its finite recovery state.
pub struct RecoveryDriver {
    engine: Box<dyn RenderTarget>,
    recovery: GraphicsRecovery,
    rebuilder: RenderTargetRebuilder,
    pending_failure: Option<GraphicsFailure>,
    terminal_failure: Option<GraphicsFailure>,
    // 记录最近一次无数据 probe 是否已经确认 surface 可呈现。
    present_probe_reopened: bool,
    // 记录连续的“probe 可用但真实 Present 遮挡”矛盾次数。
    present_probe_conflicts: u8,
    rebuild_request: RebuildRequest,
    width: i32,
    height: i32,
    shutdown: bool,
    #[cfg(feature = "test-harness")]
    test_faults: Option<GraphicsFaultSignal>,
}

impl RecoveryDriver {
    /// 使用初始渲染目标和重建回调创建恢复驱动器。
    pub fn new(engine: Box<dyn RenderTarget>, rebuilder: RenderTargetRebuilder) -> Self {
        Self {
            engine,
            recovery: GraphicsRecovery::new(true),
            rebuilder,
            pending_failure: None,
            terminal_failure: None,
            // 新建 surface 尚未经历遮挡退出 probe。
            present_probe_reopened: false,
            // 新建 surface 没有协议矛盾历史。
            present_probe_conflicts: 0,
            rebuild_request: RebuildRequest::default(),
            width: 0,
            height: 0,
            shutdown: false,
            #[cfg(feature = "test-harness")]
            test_faults: None,
        }
    }

    /// Attaches the shared rebuild request consumed at each frame boundary.
    pub(crate) fn with_rebuild_request(mut self, request: RebuildRequest) -> Self {
        self.rebuild_request = request;
        self
    }

    /// Records the already-initialized engine's current logical extent.
    pub fn with_extent(mut self, width: i32, height: i32) -> Self {
        self.width = width.max(1);
        self.height = height.max(1);
        self
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn with_test_fault_signal(mut self, signal: GraphicsFaultSignal) -> Self {
        signal.attach_recovery_driver();
        self.test_faults = Some(signal);
        self
    }

    // 清除只属于当前 surface 代际的遮挡协议观测。
    fn reset_present_probe_conflicts(&mut self) {
        // 新代际或成功 Present 不得继承旧 probe 事实。
        self.present_probe_reopened = false;
        // 连续矛盾计数同样按 surface 代际清零。
        self.present_probe_conflicts = 0;
    }

    // 记录无数据 present probe 对当前 surface 可用性的最新判断。
    fn observe_present_test(&mut self, result: &Result<PresentTestResult, Error>) {
        // 只按结构化结果更新状态，不解析平台错误文本。
        match result {
            // 可呈现 probe 允许下一次真实 Present 验证协议是否一致。
            Ok(PresentTestResult::Presentable) => self.present_probe_reopened = true,
            // probe 仍遮挡属于健康状态，并中断连续矛盾序列。
            Ok(PresentTestResult::Occluded) => self.reset_present_probe_conflicts(),
            // probe 自身失败由统一失败路径处理，不保留旧可呈现事实。
            Err(_) => self.reset_present_probe_conflicts(),
        }
    }

    fn record_failure(&mut self, mut failure: GraphicsFailure) {
        // platform 已经成功重建 Surface 时只保留当前帧 dirty，不再重复销毁整个 recipe。
        if matches!(failure, GraphicsFailure::SurfaceChanged(_)) {
            self.reset_present_probe_conflicts();
            return;
        }
        // Occlusion is a healthy swapchain availability state. Rebuilding a
        // surface cannot make another window stop covering this one.
        if matches!(failure, GraphicsFailure::Occluded(_)) {
            // 没有成功 probe 时仍按普通遮挡处理，不触发恢复。
            if !self.present_probe_reopened {
                // 普通遮挡只交给窗口调度器的退避 probe。
                return;
            }
            // 一个成功 probe 只允许验证紧随其后的一次真实 Present。
            self.present_probe_reopened = false;
            // 记录连续矛盾，饱和计数避免长时间运行回绕。
            self.present_probe_conflicts = self.present_probe_conflicts.saturating_add(1);
            // 短暂竞态尚不足以证明 surface 失效。
            if self.present_probe_conflicts < MAX_PRESENT_PROBE_CONFLICTS {
                // 保持现有遮挡退避，不提前重建图形资源。
                return;
            }
            // 达到门槛后清零当前代际计数，后续恢复拥有新的观测周期。
            self.present_probe_conflicts = 0;
            // 记录稳定的生产诊断，明确区分 probe 与真实 Present 两个阶段。
            tracing::warn!(
                "graphics recovery: present probe succeeded but real Present remained occluded; selecting Software fallback"
            );
            // 该矛盾已证明当前显示输出不兼容硬件 swapchain，跳过其它 GPU recipe。
            self.recovery.prefer_software();
            // probe 与真实 Present 持续矛盾说明当前输出 surface 已不可用。
            failure = GraphicsFailure::SurfaceLost(Error::new(
                // 使用既有 surface-lost 分类进入有界整后端恢复序列。
                crate::core::Errc::GraphicsSurfaceLost,
                // 保留稳定诊断文本，避免依赖具体原生错误字符串。
                "present probe reported available but repeated presentation remained occluded",
            ));
        } else {
            // 其它失败会终止遮挡协议观测，避免跨故障类型累计。
            self.reset_present_probe_conflicts();
        }
        // The first failure identifies the frame that was not committed.  Do
        // not overwrite it with secondary cleanup noise before recovery gets
        // a frame-boundary chance to act.
        if self.terminal_failure.is_none() && self.pending_failure.is_none() {
            self.pending_failure = Some(failure);
        }
    }

    fn recover_before_frame(&mut self) -> Option<RenderOutcome> {
        if let Some(failure) = &self.terminal_failure {
            return Some(RenderOutcome::Failed(failure.clone()));
        }
        let failure = self.pending_failure.take()?;
        let action = self.recovery.on_failure(&failure);
        match action {
            GraphicsRecoveryAction::Abort | GraphicsRecoveryAction::AbortOutOfMemory => {
                self.terminal_failure = Some(failure.clone());
                return Some(RenderOutcome::Failed(failure));
            }
            GraphicsRecoveryAction::RebuildSurface
            | GraphicsRecoveryAction::RebuildRecipe
            | GraphicsRecoveryAction::TryNextRecipe
            | GraphicsRecoveryAction::UseSoftware => {}
        }

        // 同一 native surface 不能假定可同时持有旧、新两个 live swapchain。
        // 先完成 checked teardown；失败时不探测 replacement，也不制造第二份资源。
        if let Err(error) = self.engine.try_shutdown() {
            let failure = GraphicsFailure::from_error(error);
            self.record_failure(failure.clone());
            return Some(RenderOutcome::Failed(failure));
        }

        match (self.rebuilder)(action, self.width.max(1), self.height.max(1)) {
            Ok(replacement) => {
                self.engine = replacement;
                // replacement 建立新 surface 代际，旧 probe 事实必须失效。
                self.reset_present_probe_conflicts();
                None
            }
            Err(error) => {
                let recovery_failure = GraphicsFailure::from_error(error);
                self.record_failure(recovery_failure.clone());
                Some(RenderOutcome::Failed(recovery_failure))
            }
        }
    }
}

impl RenderTarget for RecoveryDriver {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.shutdown = false;
        self.terminal_failure = None;
        self.width = width.max(1);
        self.height = height.max(1);
        if let Err(error) = self.engine.initialize(self.width, self.height) {
            self.record_failure(GraphicsFailure::from_error(error.clone()));
            return Err(error);
        }
        Ok(())
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if self.shutdown {
            return Ok(());
        }
        self.engine.try_shutdown()?;
        self.shutdown = true;
        self.pending_failure = None;
        self.terminal_failure = None;
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        if let Some(failure) = &self.terminal_failure {
            return Err(failure.error().clone());
        }
        // WindowDriver 已把零尺寸 Surface 置为暂停；恢复层不得创建 1x1 替身。
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        // A failed resize invalidates the old surface just as much as a
        // failed present. Recovery must rebuild for the requested extent,
        // not silently recreate the previous one.
        self.width = width;
        self.height = height;
        if let Err(error) = self.engine.resize(width, height) {
            self.record_failure(GraphicsFailure::from_error(error.clone()));
            return Err(error);
        }
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        #[cfg(feature = "test-harness")]
        if self.pending_failure.is_none()
            && self.terminal_failure.is_none()
            && self
                .test_faults
                .as_ref()
                .is_some_and(GraphicsFaultSignal::take_device_lost)
        {
            // 先尝试把注入送到真实 backend；原生 adapter 会在最终 present 预检报告失败。
            match self.engine.inject_graphics_device_lost_for_test() {
                Ok(()) => {}
                Err(error) if error.code() == crate::core::Errc::NotImplemented => {
                    // 未接入薄 RHI 的测试 backend 保留原有包装器级兼容语义。
                    let failure = GraphicsFailure::DeviceLost(Error::new(
                        crate::core::Errc::GraphicsDeviceLost,
                        "test-harness injected graphics device loss",
                    ));
                    self.record_failure(failure.clone());
                    return RenderOutcome::Failed(failure);
                }
                Err(error) => {
                    // 真实 adapter 注入入口的其它错误也必须进入同一恢复 FSM。
                    let failure = GraphicsFailure::from_error(error);
                    self.record_failure(failure.clone());
                    return RenderOutcome::Failed(failure);
                }
            }
        }
        #[cfg(feature = "test-harness")]
        if self.pending_failure.is_none()
            && self.terminal_failure.is_none()
            && self
                .test_faults
                .as_ref()
                .is_some_and(GraphicsFaultSignal::take_surface_lost)
        {
            // 让原生 adapter 在下一次 RHI acquire 返回 GraphicsSurfaceLost。
            match self.engine.inject_graphics_surface_lost_for_test() {
                Ok(()) => {}
                Err(error) if error.code() == crate::core::Errc::NotImplemented => {
                    // 未接入薄 RHI 的测试 backend 保留包装器级兼容回退语义。
                    let failure = GraphicsFailure::SurfaceLost(Error::new(
                        crate::core::Errc::GraphicsSurfaceLost,
                        "test-harness injected graphics surface loss",
                    ));
                    self.record_failure(failure.clone());
                    return RenderOutcome::Failed(failure);
                }
                Err(error) => {
                    // 真实 adapter 注入入口的其它错误也必须进入同一恢复 FSM。
                    let failure = GraphicsFailure::from_error(error);
                    self.record_failure(failure.clone());
                    return RenderOutcome::Failed(failure);
                }
            }
        }
        // An external recovery registration (e.g. a Diagnostics handler) may
        // request the bounded rebuild sequence at the next frame boundary.
        // record_failure keeps the first unhandled failure; the request is
        // consumed once and joins the existing recovery state machine.
        if self.rebuild_request.take() {
            self.record_failure(GraphicsFailure::DeviceLost(Error::new(
                crate::core::Errc::GraphicsDeviceLost,
                "recovery handler requested graphics rebuild",
            )));
        }
        if let Some(outcome) = self.recover_before_frame() {
            return outcome;
        }
        let outcome = self.engine.begin_frame(strategy);
        if let RenderOutcome::Failed(failure) = &outcome {
            self.record_failure(failure.clone());
        }
        outcome
    }

    fn end_frame(&mut self, present_damage: &DamageRegion) -> RenderOutcome {
        let outcome = self.engine.end_frame(present_damage);
        match &outcome {
            RenderOutcome::Present(_) => {
                // 成功提交关闭本次有界恢复 episode。
                self.recovery.on_presented();
                // 成功提交同时证明当前 surface 协议恢复一致。
                self.reset_present_probe_conflicts();
            }
            RenderOutcome::PresentPending(_) => {}
            RenderOutcome::FrameReady(_) => {
                let failure = GraphicsFailure::from_error(Error::new(
                    crate::core::Errc::InvalidState,
                    "end_frame returned FrameReady instead of final presentation",
                ));
                self.record_failure(failure.clone());
                return RenderOutcome::Failed(failure);
            }
            RenderOutcome::Failed(failure) => self.record_failure(failure.clone()),
            RenderOutcome::Idle => {}
        }
        outcome
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        let result = self.engine.test_present();
        // 在错误登记前保存 probe 结果，确保失败会清除旧可呈现事实。
        self.observe_present_test(&result);
        if let Err(error) = &result {
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        result
    }

    fn external_present_succeeded(&mut self) {
        self.engine.external_present_succeeded();
        self.recovery.on_presented();
        // 外部 presenter 的成功提交同样结束遮挡矛盾观测。
        self.reset_present_probe_conflicts();
    }

    fn external_present_failed(&mut self, error: Error) {
        self.engine.external_present_failed(error.clone());
        self.record_failure(GraphicsFailure::from_error(error));
    }

    fn note_presented_at(&mut self, now: Instant) {
        self.engine.note_presented_at(now);
    }

    fn idle_resource_deadline(&self) -> Option<Instant> {
        self.engine.idle_resource_deadline()
    }

    fn release_idle_resources(&mut self, now: Instant) {
        self.engine.release_idle_resources(now);
    }

    fn has_terminal_failure(&self) -> bool {
        self.terminal_failure.is_some()
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.engine.canvas_2d()
    }

    fn logical_extent(&mut self) -> (i32, i32) {
        self.engine.logical_extent()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.engine.capabilities()
    }

    fn raster_pipeline(&self) -> crate::draw::renderer::RasterPipeline {
        self.engine.raster_pipeline()
    }

    fn dpi(&self) -> f32 {
        self.engine.dpi()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.engine.device_pixel_ratio()
    }

    fn orientation(&self) -> crate::draw::geometry::spatial::Orientation {
        self.engine.orientation()
    }

    // RecoveryDriver 保留检查式 Picture 创建结果供场景失败链消费。
    fn try_create_offscreen(
        // 借用恢复驱动持有的唯一 engine owner。
        &mut self,
        // 接收 Picture 的逻辑宽度。
        width: i32,
        // 接收 Picture 的逻辑高度。
        height: i32,
        // 保留正常无资源或 typed allocation/device failure。
    ) -> Result<Option<ImageHandle>, Error> {
        // 不在恢复包装器内把资源失败降级为 None。
        self.engine.try_create_offscreen(width, height)
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.engine.try_destroy_offscreen(handle)
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.engine.offscreen_canvas(handle)
    }

    fn blit_offscreen_to_canvas(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
        canvas: &mut dyn Canvas2D,
    ) {
        self.engine
            .blit_offscreen_to_canvas(handle, src_rect, dst_rect, canvas);
    }

    fn copy_offscreen_pixels(&self, handle: &ImageHandle) -> Option<(Vec<u32>, i32)> {
        self.engine.copy_offscreen_pixels(handle)
    }

    fn copy_frame_pixels(&self) -> Option<(Vec<u32>, i32)> {
        self.engine.copy_frame_pixels()
    }

    // RecoveryDriver 保留 live engine 的真实 backdrop blur 能力。
    fn supports_backdrop_blur(&self) -> bool {
        // 能力查询不触发恢复动作或资源事务。
        self.engine.supports_backdrop_blur()
    }

    fn snapshot_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 先保留底层原始结果，避免把错误文本或错误码重建一遍。
        let result = self.engine.snapshot_overlay_backdrop();
        // backdrop 失败发生在 begin_frame 外，也必须登记到下一帧恢复边界。
        if let Err(error) = &result {
            // 复用统一分类保存首个未处理失败。
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        // 向 ScenePipeline 原样返回 typed result。
        result
    }

    // 对已捕获背景执行 blur，并把无帧事务失败登记到统一恢复状态机。
    fn blur_overlay_backdrop(&mut self, region: Rect, radius: f32) -> Result<bool, Error> {
        // 先保留底层原始结果，避免重建错误分类。
        let result = self.engine.blur_overlay_backdrop(region, radius);
        // blur 的资源、提交或设备失败必须驱动下一帧有界恢复。
        if let Err(error) = &result {
            // 复用统一分类保存首个未处理失败。
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        // 向上层原样返回是否执行及 typed failure。
        result
    }

    fn restore_overlay_backdrop(&mut self) -> Result<bool, Error> {
        // 先保留底层原始结果，避免把错误文本或错误码重建一遍。
        let result = self.engine.restore_overlay_backdrop();
        // restore 位于 begin_frame 之后，失败仍需驱动下一帧有界恢复。
        if let Err(error) = &result {
            // 复用统一分类保存首个未处理失败。
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        // 向 ScenePipeline 原样返回 typed result。
        result
    }

    fn release_overlay_backdrop(&mut self) -> Result<(), Error> {
        // 先保留底层原始结果，确保 owner-thread 清理错误不丢失。
        let result = self.engine.release_overlay_backdrop();
        // release 可能发生在 idle 判断前，同样必须进入恢复 FSM。
        if let Err(error) = &result {
            // 复用统一分类保存首个未处理失败。
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        // 向 ScenePipeline 原样返回 typed result。
        result
    }

    fn has_overlay_backdrop(&self) -> bool {
        self.engine.has_overlay_backdrop()
    }

    fn try_execute_encoded_picture(
        &mut self,
        handle: &ImageHandle,
        encoder: &FrameEncoder,
    ) -> Result<EncodedPictureExecution, Error> {
        self.engine.try_execute_encoded_picture(handle, encoder)
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        let result = self.engine.try_execute_encoded_frame(encoder);
        if let Err(error) = &result {
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        result
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.engine.try_begin_offscreen_paint(handle)
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.engine.try_flush_offscreen_paint(handle)
    }

    fn try_end_offscreen_paint(&mut self) -> Result<(), Error> {
        self.engine.try_end_offscreen_paint()
    }

    fn try_blit_offscreen_src(
        &mut self,
        handle: &ImageHandle,
        src_rect: Rect,
        dst_rect: Rect,
    ) -> Result<(), Error> {
        self.engine
            .try_blit_offscreen_src(handle, src_rect, dst_rect)
    }

    fn try_blur_offscreen(
        &mut self,
        handle: &ImageHandle,
        region: Rect,
        radius: f32,
    ) -> Result<(), Error> {
        self.engine.try_blur_offscreen(handle, region, radius)
    }

    fn memory_usage(&self) -> usize {
        self.engine.memory_usage()
    }

    fn diagnose_memory(&self) {
        self.engine.diagnose_memory();
    }
}

impl Drop for RecoveryDriver {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            // 主关闭路径首失败已由窗口操作边界上报；Drop 重试失败走边界观察。
            crate::diagnostics::observe_boundary_error("recovery_driver/drop", &error);
        }
    }
}

// 确定性验证共享 device loss 仍由两个窗口各自消费一次，不引入全局恢复协调。
#[cfg(feature = "graphics-parity-test")]
pub(crate) fn run_multi_window_device_loss_contract_test() {
    use crate::core::Errc;
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use std::sync::atomic::AtomicUsize;

    struct WindowTarget {
        shared_lost: Arc<AtomicBool>,
        canvas: NoopCanvas2D,
    }

    impl RenderTarget for WindowTarget {
        fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
            Ok(())
        }

        fn try_shutdown(&mut self) -> Result<(), Error> {
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
            Ok(())
        }

        fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
            if self.shared_lost.load(Ordering::Acquire) {
                return RenderOutcome::Failed(GraphicsFailure::DeviceLost(Error::new(
                    Errc::GraphicsDeviceLost,
                    "shared graphics device lost in deterministic window test",
                )));
            }
            RenderOutcome::FrameReady(DamageRegion::full())
        }

        fn end_frame(&mut self, _present_damage: &DamageRegion) -> RenderOutcome {
            RenderOutcome::Present(DamageRegion::full())
        }

        fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
            &mut self.canvas
        }
    }

    fn driver(shared_lost: Arc<AtomicBool>, rebuilds: Arc<AtomicUsize>) -> RecoveryDriver {
        let rebuilder = Box::new(move |_action, _width, _height| {
            rebuilds.fetch_add(1, Ordering::AcqRel);
            Ok(Box::new(WindowTarget {
                shared_lost: Arc::new(AtomicBool::new(false)),
                canvas: NoopCanvas2D,
            }) as Box<dyn RenderTarget>)
        });
        RecoveryDriver::new(
            Box::new(WindowTarget {
                shared_lost,
                canvas: NoopCanvas2D,
            }),
            rebuilder,
        )
        .with_extent(64, 64)
    }

    let shared_lost = Arc::new(AtomicBool::new(true));
    let first_rebuilds = Arc::new(AtomicUsize::new(0));
    let second_rebuilds = Arc::new(AtomicUsize::new(0));
    let mut first = driver(Arc::clone(&shared_lost), Arc::clone(&first_rebuilds));
    let mut second = driver(shared_lost, Arc::clone(&second_rebuilds));

    for window in [&mut first, &mut second] {
        assert!(matches!(
            window.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
        ));
        assert!(matches!(
            window.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert!(matches!(
            window.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
    }
    assert_eq!(first_rebuilds.load(Ordering::Acquire), 1);
    assert_eq!(second_rebuilds.load(Ordering::Acquire), 1);
}
