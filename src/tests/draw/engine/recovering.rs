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
    checked_shutdown_failures: Option<Rc<std::cell::Cell<usize>>>,
}

impl EndFailingEngine {
    fn new(failure: GraphicsFailure) -> Self {
        Self {
            inner: NullEngine::new(),
            failure,
            frame_failure: None,
            shutdowns: None,
            checked_shutdown_failures: None,
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

    fn with_checked_shutdown_failures(mut self, failures: Rc<std::cell::Cell<usize>>) -> Self {
        self.checked_shutdown_failures = Some(failures);
        self
    }
}

impl GraphicsEngine for EndFailingEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if let Some(failures) = &self.checked_shutdown_failures {
            let remaining = failures.get();
            if remaining > 0 {
                failures.set(remaining - 1);
                return Err(Error::new(
                    Errc::InvalidState,
                    "injected checked engine shutdown failure",
                ));
            }
        }
        self.inner.try_shutdown()?;
        if let Some(shutdowns) = &self.shutdowns {
            shutdowns.set(shutdowns.get() + 1);
        }
        Ok(())
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
fn recovery_retains_the_old_engine_when_checked_teardown_blocks_replacement() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let checked_shutdown_failures = Rc::new(std::cell::Cell::new(1));
    let replacement_shutdowns = Rc::new(std::cell::Cell::new(0));
    let recorded_replacement_shutdowns = Rc::clone(&replacement_shutdowns);
    let mut engine = RecoveringGraphicsEngine::new(
        Box::new(
            EndFailingEngine::new(surface_lost())
                .with_checked_shutdown_failures(Rc::clone(&checked_shutdown_failures)),
        ),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(
                EndFailingEngine::new(surface_lost())
                    .with_shutdown_counter(Rc::clone(&recorded_replacement_shutdowns)),
            ))
        }),
    );
    engine.initialize(4, 3).expect("initial engine");
    let _ = engine.end_frame(&DamageRegion::full());

    let outcome = engine.begin_frame(UpdateStrategy::FullRedraw);
    let RenderOutcome::Failed(GraphicsFailure::Other(error)) = outcome else {
        panic!("failed checked teardown must remain a typed recovery failure");
    };
    assert_eq!(error.code(), Errc::InvalidState);
    assert_eq!(error.message(), "injected checked engine shutdown failure");
    assert_eq!(checked_shutdown_failures.get(), 0);
    assert_eq!(replacement_shutdowns.get(), 1);

    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(
        actions.borrow().as_slice(),
        [
            RecoveryAction::RebuildSurface,
            RecoveryAction::RebuildRecipe
        ]
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

    engine.try_shutdown().expect("checked shutdown");
    drop(engine);

    assert_eq!(shutdowns.get(), 1);
}
