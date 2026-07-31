//! Frame-boundary graphics recovery wrapper.
//!
//! The wrapped engine owns a live, thread-affine graphics context.  This
//! wrapper never probes or replaces it while a frame is being recorded: a
//! typed failure is returned first so the caller retains dirty state, then the
//! next `begin_frame` runs exactly one bounded recovery action.

use crate::core::{Error, Rect};
use crate::draw::backend::DamageRegion;
use crate::draw::command::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::geometry::types::ImageHandle;
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::draw::renderer::{GraphicsFailure, GraphicsRecovery, RecoveryAction, RenderOutcome};
use crate::draw::{Canvas2D, GraphicsCapabilities, RenderTarget, UpdateStrategy};
use crate::native::traits::present::PresentTestResult;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

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
    Box<dyn FnMut(RecoveryAction, i32, i32) -> Result<Box<dyn RenderTarget>, Error>>;

/// A render target plus its finite recovery state.
pub struct RecoveryDriver {
    engine: Box<dyn RenderTarget>,
    recovery: GraphicsRecovery,
    rebuilder: RenderTargetRebuilder,
    pending_failure: Option<GraphicsFailure>,
    terminal_failure: Option<GraphicsFailure>,
    rebuild_request: RebuildRequest,
    width: i32,
    height: i32,
    shutdown: bool,
    #[cfg(feature = "test-harness")]
    test_faults: Option<GraphicsFaultSignal>,
}

impl RecoveryDriver {
    pub fn new(engine: Box<dyn RenderTarget>, rebuilder: RenderTargetRebuilder) -> Self {
        Self {
            engine,
            recovery: GraphicsRecovery::new(true),
            rebuilder,
            pending_failure: None,
            terminal_failure: None,
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

    fn record_failure(&mut self, failure: GraphicsFailure) {
        // Occlusion is a healthy swapchain availability state. Rebuilding a
        // surface cannot make another window stop covering this one.
        if matches!(failure, GraphicsFailure::Occluded(_)) {
            return;
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
            RecoveryAction::Abort | RecoveryAction::AbortOutOfMemory => {
                self.terminal_failure = Some(failure.clone());
                return Some(RenderOutcome::Failed(failure));
            }
            RecoveryAction::RebuildSurface
            | RecoveryAction::RebuildRecipe
            | RecoveryAction::TryNextRecipe
            | RecoveryAction::UseSoftware => {}
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
        let width = width.max(1);
        let height = height.max(1);
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
            let failure = GraphicsFailure::DeviceLost(Error::new(
                crate::core::Errc::GraphicsDeviceLost,
                "test-harness injected graphics device loss",
            ));
            self.record_failure(failure.clone());
            return RenderOutcome::Failed(failure);
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
            RenderOutcome::Present(_) => self.recovery.on_presented(),
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
        if let Err(error) = &result {
            self.record_failure(GraphicsFailure::from_error(error.clone()));
        }
        result
    }

    fn external_present_succeeded(&mut self) {
        self.engine.external_present_succeeded();
        self.recovery.on_presented();
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

    fn create_offscreen(&mut self, width: i32, height: i32) -> Option<ImageHandle> {
        self.engine.create_offscreen(width, height)
    }

    fn destroy_offscreen(&mut self, handle: ImageHandle) {
        self.engine.destroy_offscreen(handle);
    }

    fn try_destroy_offscreen(&mut self, handle: ImageHandle) -> Result<(), Error> {
        self.engine.try_destroy_offscreen(handle)
    }

    fn offscreen_canvas(&mut self, handle: &ImageHandle) -> Option<&mut dyn Canvas2D> {
        self.engine.offscreen_canvas(handle)
    }

    fn blit_offscreen(&mut self, handle: &ImageHandle, dst_rect: Rect) {
        self.engine.blit_offscreen(handle, dst_rect);
    }

    fn blit_offscreen_src(&mut self, handle: &ImageHandle, src_rect: Rect, dst_rect: Rect) {
        self.engine.blit_offscreen_src(handle, src_rect, dst_rect);
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

    fn snapshot_overlay_backdrop(&mut self) -> bool {
        self.engine.snapshot_overlay_backdrop()
    }

    fn restore_overlay_backdrop(&mut self) -> bool {
        self.engine.restore_overlay_backdrop()
    }

    fn release_overlay_backdrop(&mut self) {
        self.engine.release_overlay_backdrop();
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

    fn begin_offscreen_paint(&mut self, handle: &ImageHandle) -> bool {
        self.engine.begin_offscreen_paint(handle)
    }

    fn try_begin_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.engine.try_begin_offscreen_paint(handle)
    }

    fn flush_offscreen_paint(&mut self, handle: &ImageHandle) {
        self.engine.flush_offscreen_paint(handle);
    }

    fn try_flush_offscreen_paint(&mut self, handle: &ImageHandle) -> Result<(), Error> {
        self.engine.try_flush_offscreen_paint(handle)
    }

    fn end_offscreen_paint(&mut self) {
        self.engine.end_offscreen_paint();
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
            tracing::error!(
                "RecoveryDriver checked shutdown failed: {}",
                error.short_what()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Errc;
    use crate::draw::backend::cpu::noop_canvas_2d::NoopCanvas2D;
    use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

    struct StubTarget {
        frames: u32,
        fail_next: bool,
        shutdown: bool,
        canvas: NoopCanvas2D,
    }

    impl StubTarget {
        fn new() -> Self {
            Self {
                frames: 0,
                fail_next: false,
                shutdown: false,
                canvas: NoopCanvas2D,
            }
        }

        fn new_failing() -> Self {
            Self {
                frames: 0,
                fail_next: true,
                shutdown: false,
                canvas: NoopCanvas2D,
            }
        }
    }

    impl RenderTarget for StubTarget {
        fn initialize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
            Ok(())
        }

        fn try_shutdown(&mut self) -> Result<(), Error> {
            self.shutdown = true;
            Ok(())
        }

        fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
            Ok(())
        }

        fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
            self.frames += 1;
            if self.fail_next {
                self.fail_next = false;
                return RenderOutcome::Failed(GraphicsFailure::DeviceLost(Error::new(
                    Errc::GraphicsDeviceLost,
                    "stub device lost",
                )));
            }
            RenderOutcome::FrameReady(DamageRegion::full())
        }

        fn end_frame(&mut self, _damage: &DamageRegion) -> RenderOutcome {
            RenderOutcome::Present(DamageRegion::full())
        }

        fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
            &mut self.canvas
        }
    }

    fn counting_rebuilder(rebuilds: &Arc<AtomicUsize>) -> RenderTargetRebuilder {
        let rebuilds = Arc::clone(rebuilds);
        Box::new(move |_action: RecoveryAction, _width: i32, _height: i32| {
            rebuilds.fetch_add(1, AtomicOrdering::SeqCst);
            Ok(Box::new(StubTarget::new()) as Box<dyn RenderTarget>)
        })
    }

    #[test]
    fn rebuild_request_runs_bounded_recovery_at_next_frame_boundary() {
        let request = RebuildRequest::default();
        let rebuilds = Arc::new(AtomicUsize::new(0));
        let mut driver =
            RecoveryDriver::new(Box::new(StubTarget::new()), counting_rebuilder(&rebuilds))
                .with_extent(100, 100)
                .with_rebuild_request(request.clone());

        // 无请求时正常推进，不触发重建。
        assert!(matches!(
            driver.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);

        // 请求后下一帧执行有界恢复序列（RebuildSurface → rebuilder 一次）。
        request.request_rebuild();
        let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
        assert!(
            matches!(outcome, RenderOutcome::FrameReady(_)),
            "recovery should rebuild and continue, got {outcome:?}"
        );
        assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);

        // 请求是一次性的：后续帧不再重复恢复。
        let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
        assert!(matches!(outcome, RenderOutcome::FrameReady(_)));
        assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);
    }

    #[test]
    fn rebuild_request_joins_an_already_pending_failure_without_double_recovery() {
        let request = RebuildRequest::default();
        let rebuilds = Arc::new(AtomicUsize::new(0));
        let mut driver =
            RecoveryDriver::new(Box::new(StubTarget::new()), counting_rebuilder(&rebuilds))
                .with_extent(100, 100)
                .with_rebuild_request(request.clone());

        // 先注入一次真实帧失败：本帧直接失败并保留 pending failure。
        let failing = Box::new(StubTarget::new_failing());
        driver.engine = failing;
        let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
        assert!(matches!(outcome, RenderOutcome::Failed(_)));
        assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 0);

        // 恢复请求被消费，但不覆盖首个未处理失败，也不产生第二次重建：
        // 下一帧只执行一次有界恢复（针对原始 device-lost 失败）。
        request.request_rebuild();
        let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
        assert!(
            matches!(outcome, RenderOutcome::FrameReady(_)),
            "recovery should rebuild once and continue, got {outcome:?}"
        );
        assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);

        // 请求是一次性的：后续帧不再重复恢复。
        let outcome = driver.begin_frame(UpdateStrategy::FullRedraw);
        assert!(matches!(outcome, RenderOutcome::FrameReady(_)));
        assert_eq!(rebuilds.load(AtomicOrdering::SeqCst), 1);
    }
}
