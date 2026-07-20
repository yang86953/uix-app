//! Frame-boundary graphics recovery wrapper.
//!
//! The wrapped engine owns a live, thread-affine graphics context.  This
//! wrapper never probes or replaces it while a frame is being recorded: a
//! typed failure is returned first so the caller retains dirty state, then the
//! next `begin_frame` runs exactly one bounded recovery action.

use crate::core::{Error, Rect};
use crate::draw::backend::DamageRegion;
#[cfg(feature = "test-harness")]
use crate::draw::engine::graphics_test_harness::GraphicsFaultSignal;
use crate::draw::engine::{GraphicsFailure, GraphicsRecovery, RecoveryAction, RenderOutcome};
use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::primitives::types::ImageHandle;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};
use crate::native::traits::present::PresentTestResult;
use std::time::Instant;

/// Recreates one initialized engine for a permitted recovery action.
///
/// The app-side closure owns the native surface and recipe cursor.  It must
/// only recreate the exact action requested here; this keeps probe policy out
/// of frame recording and makes fallback order auditable.
pub type GraphicsEngineRebuilder =
    Box<dyn FnMut(RecoveryAction, i32, i32) -> Result<Box<dyn GraphicsEngine>, Error>>;

/// A graphics engine plus its finite recovery state.
pub struct RecoveringGraphicsEngine {
    engine: Box<dyn GraphicsEngine>,
    recovery: GraphicsRecovery,
    rebuilder: GraphicsEngineRebuilder,
    pending_failure: Option<GraphicsFailure>,
    terminal_failure: Option<GraphicsFailure>,
    width: i32,
    height: i32,
    shutdown: bool,
    #[cfg(feature = "test-harness")]
    test_faults: Option<GraphicsFaultSignal>,
}

impl RecoveringGraphicsEngine {
    pub fn new(engine: Box<dyn GraphicsEngine>, rebuilder: GraphicsEngineRebuilder) -> Self {
        Self {
            engine,
            recovery: GraphicsRecovery::new(true),
            rebuilder,
            pending_failure: None,
            terminal_failure: None,
            width: 0,
            height: 0,
            shutdown: false,
            #[cfg(feature = "test-harness")]
            test_faults: None,
        }
    }

    /// Records the already-initialized engine's current logical extent.
    pub fn with_extent(mut self, width: i32, height: i32) -> Self {
        self.width = width.max(1);
        self.height = height.max(1);
        self
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn with_test_fault_signal(mut self, signal: GraphicsFaultSignal) -> Self {
        signal.attach_recovering_engine();
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

impl GraphicsEngine for RecoveringGraphicsEngine {
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

    fn raster_pipeline(&self) -> crate::draw::traits::RasterPipeline {
        self.engine.raster_pipeline()
    }

    fn dpi(&self) -> f32 {
        self.engine.dpi()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.engine.device_pixel_ratio()
    }

    fn orientation(&self) -> crate::draw::spatial::Orientation {
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

impl Drop for RecoveringGraphicsEngine {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            crate::core::log::error_fn(format_args!(
                "RecoveringGraphicsEngine checked shutdown failed: {}",
                error.short_what()
            ));
        }
    }
}
