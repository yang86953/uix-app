//! Frame-boundary graphics recovery wrapper.
//!
//! The wrapped engine owns a live, thread-affine graphics context.  This
//! wrapper never probes or replaces it while a frame is being recorded: a
//! typed failure is returned first so the caller retains dirty state, then the
//! next `begin_frame` runs exactly one bounded recovery action.

use crate::core::{Error, Rect};
use crate::draw::backend::DamageRegion;
use crate::draw::engine::{GraphicsFailure, GraphicsRecovery, RecoveryAction, RenderOutcome};
use crate::draw::pipeline::{EncodedFrameExecution, EncodedPictureExecution, FrameEncoder};
use crate::draw::primitives::types::ImageHandle;
use crate::draw::traits::{Canvas2D, GraphicsCapabilities, GraphicsEngine, UpdateStrategy};

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
        }
    }

    /// Records the already-initialized engine's current logical extent.
    pub fn with_extent(mut self, width: i32, height: i32) -> Self {
        self.width = width.max(1);
        self.height = height.max(1);
        self
    }

    fn record_failure(&mut self, failure: GraphicsFailure) {
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

        match (self.rebuilder)(action, self.width.max(1), self.height.max(1)) {
            Ok(engine) => {
                self.engine.shutdown();
                self.engine = engine;
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

    fn shutdown(&mut self) {
        if self.shutdown {
            return;
        }
        self.shutdown = true;
        self.pending_failure = None;
        self.terminal_failure = None;
        self.engine.shutdown();
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        if let Some(failure) = &self.terminal_failure {
            return Err(failure.error().clone());
        }
        let width = width.max(1);
        let height = height.max(1);
        if let Err(error) = self.engine.resize(width, height) {
            self.record_failure(GraphicsFailure::from_error(error.clone()));
            return Err(error);
        }
        self.width = width;
        self.height = height;
        Ok(())
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
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

    fn external_present_succeeded(&mut self) {
        self.engine.external_present_succeeded();
        self.recovery.on_presented();
    }

    fn external_present_failed(&mut self, error: Error) {
        self.engine.external_present_failed(error.clone());
        self.record_failure(GraphicsFailure::from_error(error));
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.engine.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.engine.capabilities()
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

    fn memory_usage(&self) -> usize {
        self.engine.memory_usage()
    }

    fn diagnose_memory(&self) {
        self.engine.diagnose_memory();
    }
}

impl Drop for RecoveringGraphicsEngine {
    fn drop(&mut self) {
        <Self as GraphicsEngine>::shutdown(self);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Errc, Error};
    use crate::draw::null_engine::NullEngine;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct EndFailingEngine {
        inner: NullEngine,
        failure: GraphicsFailure,
        frame_failure: Option<Error>,
        shutdowns: Option<Rc<std::cell::Cell<usize>>>,
    }

    impl EndFailingEngine {
        fn new(failure: GraphicsFailure) -> Self {
            Self {
                inner: NullEngine::new(),
                failure,
                frame_failure: None,
                shutdowns: None,
            }
        }

        fn with_shutdown_counter(mut self, shutdowns: Rc<std::cell::Cell<usize>>) -> Self {
            self.shutdowns = Some(shutdowns);
            self
        }

        fn with_frame_failure(mut self, failure: Error) -> Self {
            self.frame_failure = Some(failure);
            self
        }
    }

    impl GraphicsEngine for EndFailingEngine {
        fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
            self.inner.initialize(width, height)
        }

        fn shutdown(&mut self) {
            self.inner.shutdown();
            if let Some(shutdowns) = &self.shutdowns {
                shutdowns.set(shutdowns.get() + 1);
            }
        }

        fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
            self.inner.resize(width, height)
        }

        fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
            self.inner.begin_frame(strategy)
        }

        fn end_frame(&mut self, _damage: &DamageRegion) -> RenderOutcome {
            RenderOutcome::Failed(self.failure.clone())
        }

        fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
            self.inner.canvas_2d()
        }

        fn try_execute_encoded_frame(
            &mut self,
            encoder: &FrameEncoder,
        ) -> Result<EncodedFrameExecution, Error> {
            match &self.frame_failure {
                Some(error) => Err(error.clone()),
                None => self.inner.try_execute_encoded_frame(encoder),
            }
        }
    }

    fn surface_lost() -> GraphicsFailure {
        GraphicsFailure::SurfaceLost(Error::new(Errc::GraphicsSurfaceLost, "test surface lost"))
    }

    #[test]
    fn frame_failure_is_retained_then_rebuilt_at_next_begin_boundary() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded_actions = Rc::clone(&actions);
        let mut engine = RecoveringGraphicsEngine::new(
            Box::new(EndFailingEngine::new(surface_lost())),
            Box::new(move |action, _, _| {
                recorded_actions.borrow_mut().push(action);
                Ok(Box::new(NullEngine::new()))
            }),
        );
        engine.initialize(4, 3).expect("initial engine");

        assert!(matches!(
            engine.end_frame(&DamageRegion::full()),
            RenderOutcome::Failed(GraphicsFailure::SurfaceLost(_))
        ));
        assert!(matches!(
            engine.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert_eq!(
            actions.borrow().as_slice(),
            [RecoveryAction::RebuildSurface]
        );
    }

    #[test]
    fn external_present_failure_is_retained_until_the_next_frame_boundary() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded_actions = Rc::clone(&actions);
        let mut engine = RecoveringGraphicsEngine::new(
            Box::new(EndFailingEngine::new(surface_lost())),
            Box::new(move |action, _, _| {
                recorded_actions.borrow_mut().push(action);
                Ok(Box::new(NullEngine::new()))
            }),
        );
        engine.initialize(4, 3).expect("initial engine");

        engine.external_present_failed(Error::new(
            Errc::GraphicsSurfaceLost,
            "injected external presenter failure",
        ));

        assert!(matches!(
            engine.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert_eq!(
            actions.borrow().as_slice(),
            [RecoveryAction::RebuildSurface]
        );
    }

    #[test]
    fn main_frame_encoder_failure_is_retained_until_the_next_frame_boundary() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded_actions = Rc::clone(&actions);
        let mut engine = RecoveringGraphicsEngine::new(
            Box::new(
                EndFailingEngine::new(surface_lost()).with_frame_failure(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "injected main FrameEncoder failure",
                )),
            ),
            Box::new(move |action, _, _| {
                recorded_actions.borrow_mut().push(action);
                Ok(Box::new(NullEngine::new()))
            }),
        );
        engine.initialize(4, 3).expect("initial engine");
        let encoder = FrameEncoder::new(4, 3).expect("main FrameEncoder");

        let error = engine
            .try_execute_encoded_frame(&encoder)
            .expect_err("injected main frame failure");
        assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
        assert!(matches!(
            engine.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert_eq!(
            actions.borrow().as_slice(),
            [RecoveryAction::RebuildSurface]
        );
    }

    #[test]
    fn out_of_memory_remains_terminal_and_does_not_invoke_rebuilder() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded_actions = Rc::clone(&actions);
        let oom = GraphicsFailure::OutOfMemory(Error::new(
            Errc::GraphicsOutOfMemory,
            "test allocation failure",
        ));
        let mut engine = RecoveringGraphicsEngine::new(
            Box::new(EndFailingEngine::new(oom)),
            Box::new(move |action, _, _| {
                recorded_actions.borrow_mut().push(action);
                Ok(Box::new(NullEngine::new()))
            }),
        );
        engine.initialize(4, 3).expect("initial engine");
        let _ = engine.end_frame(&DamageRegion::full());

        assert!(matches!(
            engine.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::Failed(GraphicsFailure::OutOfMemory(_))
        ));
        assert!(matches!(
            engine.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::Failed(GraphicsFailure::OutOfMemory(_))
        ));
        assert!(actions.borrow().is_empty());
    }

    #[test]
    fn recovery_escalates_surface_recipe_next_candidate_then_software() {
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded_actions = Rc::clone(&actions);
        let mut engine = RecoveringGraphicsEngine::new(
            Box::new(EndFailingEngine::new(surface_lost())),
            Box::new(move |action, _, _| {
                recorded_actions.borrow_mut().push(action);
                if action == RecoveryAction::UseSoftware {
                    Ok(Box::new(NullEngine::new()))
                } else {
                    Err(Error::new(Errc::PlatformError, "injected recovery failure"))
                }
            }),
        );
        engine.initialize(4, 3).expect("initial engine");
        let _ = engine.end_frame(&DamageRegion::full());

        for _ in 0..3 {
            assert!(matches!(
                engine.begin_frame(UpdateStrategy::FullRedraw),
                RenderOutcome::Failed(_)
            ));
        }
        assert!(matches!(
            engine.begin_frame(UpdateStrategy::FullRedraw),
            RenderOutcome::FrameReady(_)
        ));
        assert_eq!(
            actions.borrow().as_slice(),
            [
                RecoveryAction::RebuildSurface,
                RecoveryAction::RebuildRecipe,
                RecoveryAction::TryNextRecipe,
                RecoveryAction::UseSoftware,
            ]
        );
    }

    #[test]
    fn shutdown_and_drop_close_the_wrapped_engine_once() {
        let shutdowns = Rc::new(std::cell::Cell::new(0));
        let mut engine = RecoveringGraphicsEngine::new(
            Box::new(
                EndFailingEngine::new(surface_lost()).with_shutdown_counter(Rc::clone(&shutdowns)),
            ),
            Box::new(|_, _, _| Ok(Box::new(NullEngine::new()))),
        );

        engine.shutdown();
        drop(engine);

        assert_eq!(shutdowns.get(), 1);
    }
}
