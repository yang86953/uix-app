use crate::draw::command::EncodedFrameExecution;
use crate::draw::renderer::recovery_driver::*;
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::draw::renderer::GraphicsFailure;
use crate::draw::{Canvas2D, RenderTarget, UpdateStrategy};
#[cfg(feature = "vulkan")]
use crate::native::graphics::vulkan::platform::context::accept_device_wait_for_shutdown;
use crate::native::traits::present::PresentTestResult;
use crate::tests::common::*;
#[cfg(feature = "vulkan")]
use ash::vk;

struct EndFailingEngine {
    inner: Renderer,
    failure: GraphicsFailure,
    frame_failure: Option<Error>,
    resize_failure: Option<Error>,
    shutdowns: Option<Rc<std::cell::Cell<usize>>>,
    checked_shutdown_failures: Option<Rc<std::cell::Cell<usize>>>,
    #[cfg(feature = "vulkan")]
    shutdown_wait_error: Option<vk::Result>,
}

impl EndFailingEngine {
    fn new(failure: GraphicsFailure) -> Self {
        Self {
            inner: Renderer::test(),
            failure,
            frame_failure: None,
            resize_failure: None,
            shutdowns: None,
            checked_shutdown_failures: None,
            #[cfg(feature = "vulkan")]
            shutdown_wait_error: None,
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

    fn with_resize_failure(mut self, failure: Error) -> Self {
        self.resize_failure = Some(failure);
        self
    }

    fn with_checked_shutdown_failures(mut self, failures: Rc<std::cell::Cell<usize>>) -> Self {
        self.checked_shutdown_failures = Some(failures);
        self
    }

    #[cfg(feature = "vulkan")]
    fn with_shutdown_wait_error(mut self, error: vk::Result) -> Self {
        self.shutdown_wait_error = Some(error);
        self
    }
}

impl RenderTarget for EndFailingEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        #[cfg(feature = "vulkan")]
        {
            if let Some(error) = self.shutdown_wait_error.take() {
                accept_device_wait_for_shutdown(Err(error))?;
            }
        }
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
        if let Some(error) = &self.resize_failure {
            return Err(error.clone());
        }
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

struct OccludedEngine {
    inner: Renderer,
    probes: Rc<std::cell::Cell<usize>>,
}

struct MaintenanceProbeEngine {
    inner: Renderer,
    deadline: Option<Instant>,
    notes: Rc<std::cell::Cell<usize>>,
    releases: Rc<std::cell::Cell<usize>>,
}

impl RenderTarget for MaintenanceProbeEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn note_presented_at(&mut self, now: Instant) {
        self.notes.set(self.notes.get() + 1);
        self.deadline = Some(now + Duration::from_millis(250));
    }

    fn idle_resource_deadline(&self) -> Option<Instant> {
        self.deadline
    }

    fn release_idle_resources(&mut self, now: Instant) {
        if self.deadline.is_some_and(|deadline| deadline <= now) {
            self.releases.set(self.releases.get() + 1);
            self.deadline = None;
        }
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        self.inner.try_execute_encoded_frame(encoder)
    }
}

impl RenderTarget for OccludedEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, strategy: UpdateStrategy) -> RenderOutcome {
        self.inner.begin_frame(strategy)
    }

    fn end_frame(&mut self, _damage: &DamageRegion) -> RenderOutcome {
        RenderOutcome::Failed(GraphicsFailure::Occluded(Error::new(
            Errc::GraphicsOccluded,
            "test window occluded",
        )))
    }

    fn test_present(&mut self) -> Result<PresentTestResult, Error> {
        self.probes.set(self.probes.get() + 1);
        Ok(PresentTestResult::Occluded)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn try_execute_encoded_frame(
        &mut self,
        encoder: &FrameEncoder,
    ) -> Result<EncodedFrameExecution, Error> {
        self.inner.try_execute_encoded_frame(encoder)
    }
}

#[test]
fn occlusion_keeps_the_healthy_engine_and_delegates_idle_probe() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let probes = Rc::new(std::cell::Cell::new(0));
    let mut engine = RecoveryDriver::new(
        Box::new(OccludedEngine {
            inner: Renderer::test(),
            probes: Rc::clone(&probes),
        }),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
        }),
    );
    engine.initialize(4, 3).expect("initial engine");

    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        RenderOutcome::Failed(GraphicsFailure::Occluded(_))
    ));
    assert_eq!(
        engine.test_present().expect("idle probe"),
        PresentTestResult::Occluded
    );
    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(probes.get(), 1);
    assert!(actions.borrow().is_empty());
}

#[test]
fn recovering_engine_delegates_idle_resource_maintenance() {
    let notes = Rc::new(std::cell::Cell::new(0));
    let releases = Rc::new(std::cell::Cell::new(0));
    let mut engine = RecoveryDriver::new(
        Box::new(MaintenanceProbeEngine {
            inner: Renderer::test(),
            deadline: None,
            notes: Rc::clone(&notes),
            releases: Rc::clone(&releases),
        }),
        Box::new(|_, _, _| Ok(Box::new(Renderer::test()))),
    );
    let presented_at = Instant::now();
    engine.note_presented_at(presented_at);
    let deadline = presented_at + Duration::from_millis(250);
    assert_eq!(engine.idle_resource_deadline(), Some(deadline));

    engine.release_idle_resources(deadline);

    assert_eq!(notes.get(), 1);
    assert_eq!(releases.get(), 1);
    assert_eq!(engine.idle_resource_deadline(), None);
}

#[test]
fn frame_failure_is_retained_then_rebuilt_at_next_begin_boundary() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let mut engine = RecoveryDriver::new(
        Box::new(EndFailingEngine::new(surface_lost())),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
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

#[cfg(feature = "test-harness")]
#[test]
fn test_harness_device_loss_fails_one_frame_before_recovery_action() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let graphics_faults = GraphicsFaultSignal::default();
    let mut engine = RecoveryDriver::new(
        Box::new(Renderer::test()),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
        }),
    )
    .with_test_fault_signal(graphics_faults.clone());
    engine.initialize(4, 3).expect("initial engine");
    graphics_faults
        .arm_device_lost()
        .expect("arm device-lost fault");

    let first = engine.begin_frame(UpdateStrategy::FullRedraw);
    let RenderOutcome::Failed(GraphicsFailure::DeviceLost(error)) = first else {
        panic!("injected fault must fail the next real frame");
    };
    assert_eq!(error.code(), Errc::GraphicsDeviceLost);
    assert!(actions.borrow().is_empty());

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
    let mut engine = RecoveryDriver::new(
        Box::new(EndFailingEngine::new(surface_lost())),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
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
    let mut engine = RecoveryDriver::new(
        Box::new(
            EndFailingEngine::new(surface_lost()).with_frame_failure(Error::new(
                Errc::GraphicsSurfaceLost,
                "injected main FrameEncoder failure",
            )),
        ),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
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
fn failed_resize_rebuilds_the_requested_extent_at_the_next_frame_boundary() {
    let rebuilds = Rc::new(RefCell::new(Vec::new()));
    let recorded_rebuilds = Rc::clone(&rebuilds);
    let mut engine = RecoveryDriver::new(
        Box::new(
            EndFailingEngine::new(surface_lost()).with_resize_failure(Error::new(
                Errc::GraphicsSurfaceLost,
                "injected resize surface loss",
            )),
        ),
        Box::new(move |action, width, height| {
            recorded_rebuilds.borrow_mut().push((action, width, height));
            Ok(Box::new(Renderer::test()))
        }),
    );
    engine.initialize(4, 3).expect("initial engine");

    let error = engine
        .resize(17, 19)
        .expect_err("injected resize failure must propagate");
    assert_eq!(error.code(), Errc::GraphicsSurfaceLost);
    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(
        rebuilds.borrow().as_slice(),
        [(RecoveryAction::RebuildSurface, 17, 19)]
    );
}

#[test]
fn recovery_does_not_build_replacement_until_checked_teardown_succeeds() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let checked_shutdown_failures = Rc::new(std::cell::Cell::new(1));
    let replacement_shutdowns = Rc::new(std::cell::Cell::new(0));
    let recorded_replacement_shutdowns = Rc::clone(&replacement_shutdowns);
    let mut engine = RecoveryDriver::new(
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
    assert_eq!(replacement_shutdowns.get(), 0);
    assert!(actions.borrow().is_empty());

    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(actions.borrow().as_slice(), [RecoveryAction::RebuildRecipe]);
}

#[cfg(feature = "vulkan")]
#[test]
fn recovery_accepts_replacement_after_vulkan_device_lost_teardown() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let shutdowns = Rc::new(std::cell::Cell::new(0));
    let mut engine = RecoveryDriver::new(
        Box::new(
            EndFailingEngine::new(GraphicsFailure::DeviceLost(Error::new(
                Errc::GraphicsDeviceLost,
                "injected Vulkan device loss",
            )))
            .with_shutdown_wait_error(vk::Result::ERROR_DEVICE_LOST)
            .with_shutdown_counter(Rc::clone(&shutdowns)),
        ),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
        }),
    );
    engine.initialize(4, 3).expect("initial engine");
    assert!(matches!(
        engine.end_frame(&DamageRegion::full()),
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
    ));

    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::FrameReady(_)
    ));
    assert_eq!(
        actions.borrow().as_slice(),
        [RecoveryAction::RebuildSurface]
    );
    assert_eq!(shutdowns.get(), 1);
    assert!(!engine.has_terminal_failure());
}

#[test]
fn out_of_memory_remains_terminal_and_does_not_invoke_rebuilder() {
    let actions = Rc::new(RefCell::new(Vec::new()));
    let recorded_actions = Rc::clone(&actions);
    let oom = GraphicsFailure::OutOfMemory(Error::new(
        Errc::GraphicsOutOfMemory,
        "test allocation failure",
    ));
    let mut engine = RecoveryDriver::new(
        Box::new(EndFailingEngine::new(oom)),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            Ok(Box::new(Renderer::test()))
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
    let mut engine = RecoveryDriver::new(
        Box::new(EndFailingEngine::new(surface_lost())),
        Box::new(move |action, _, _| {
            recorded_actions.borrow_mut().push(action);
            if action == RecoveryAction::UseSoftware {
                Ok(Box::new(Renderer::test()))
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
    let mut engine = RecoveryDriver::new(
        Box::new(
            EndFailingEngine::new(surface_lost()).with_shutdown_counter(Rc::clone(&shutdowns)),
        ),
        Box::new(|_, _, _| Ok(Box::new(Renderer::test()))),
    );

    engine.try_shutdown().expect("checked shutdown");
    drop(engine);

    assert_eq!(shutdowns.get(), 1);
}
