use crate::app::clock::AppClock;
use crate::app::event_loop::event_loop::run_window_session_loop_with_clock;
use crate::app::shell::application::graphics_recovery_rebuilder;
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::draw::renderer::RenderMetrics;
use crate::draw::renderer::{GraphicsFailure, RecoveryDriver};
use crate::draw::{Canvas2D, GraphicsCapabilities, UpdateStrategy};
use crate::native::factory::GraphicsRecipe;
use crate::native::test_harness::{FakePlatform, FakeWindow};
use crate::native::traits::present::NativeSurfaceHandle;
use crate::tests::common::*;
use crate::ui::view::ViewNode;
use crate::ui::widgets::Container;

struct DeviceLostEngine {
    inner: Renderer,
}

impl DeviceLostEngine {
    fn new() -> Self {
        Self {
            inner: Renderer::test(),
        }
    }
}

impl RenderTarget for DeviceLostEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(Error::new(
            Errc::GraphicsDeviceLost,
            "injected device loss",
        )))
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        GraphicsCapabilities::backend_managed_full_redraw()
    }
}

#[derive(Debug)]
struct SteppingClock {
    now: Mutex<Instant>,
    step: Duration,
}

impl SteppingClock {
    fn new(now: Instant, step: Duration) -> Arc<Self> {
        Arc::new(Self {
            now: Mutex::new(now),
            step,
        })
    }
}

impl AppClock for SteppingClock {
    fn now(&self) -> Instant {
        let mut now = self.now.lock().unwrap_or_else(|error| error.into_inner());
        let current = *now;
        *now += self.step;
        current
    }
}

const WIDTH: i32 = 7;
const HEIGHT: i32 = 5;

fn recovering_engine(actions: Arc<Mutex<Vec<RecoveryAction>>>) -> RecoveryDriver {
    let mut failing_engine = DeviceLostEngine::new();
    failing_engine
        .initialize(WIDTH, HEIGHT)
        .expect("initial failing engine");

    let recorded_actions = Arc::clone(&actions);
    let mut software_rebuilder = graphics_recovery_rebuilder(
        // SAFETY: 测试拦截全部 GPU 恢复动作，只有 Software 分支会使用该占位 surface。
        unsafe { NativeSurfaceHandle::from_raw(std::ptr::null_mut()) },
        GraphicsBackend::Auto,
        GraphicsRecipe::new(
            GraphicsBackend::D3d11,
            RasterMode::GpuNative,
            PresentMode::Swapchain,
        ),
    );
    RecoveryDriver::new(
        Box::new(failing_engine),
        Box::new(move |action, width, height| {
            recorded_actions
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .push(action);
            if action == RecoveryAction::UseSoftware {
                software_rebuilder(action, width, height)
            } else {
                Err(Error::new(
                    Errc::GraphicsDeviceLost,
                    "injected GPU recovery failure",
                ))
            }
        }),
    )
    .with_extent(WIDTH, HEIGHT)
}

struct RecoveryRun {
    platform: FakePlatform,
    window: FakeWindow,
    session: WindowSession,
    actions: Arc<Mutex<Vec<RecoveryAction>>>,
    metrics: Cell<RenderMetrics>,
    status: i32,
}

fn run_recovery(present_error: Option<Error>) -> RecoveryRun {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let recovering = recovering_engine(Arc::clone(&actions));

    let mut platform = FakePlatform::new();
    platform.event_source.state.exit_after_blocking_calls = Some(1);
    let mut window = FakeWindow::new(1, "software recovery", WIDTH, HEIGHT);
    window.presenter.state.present_error = present_error;
    let mut session = WindowSession::from_root(
        ViewNode::leaf(Container::new()),
        Box::new(recovering),
        WIDTH,
        HEIGHT,
    );
    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::default());
    let metrics = Cell::new(RenderMetrics::default());

    let status = run_window_session_loop_with_clock(
        &mut platform,
        &mut window,
        &mut session,
        &font_service,
        &image_service,
        &theme,
        SteppingClock::new(Instant::now(), Duration::from_millis(128)),
        &debug_mode,
        &cursor_pos,
        Some(&metrics),
        |_| None,
        |_| false,
        |_, _, _| {},
    );

    RecoveryRun {
        platform,
        window,
        session,
        actions,
        metrics,
        status,
    }
}

fn expected_recovery_actions() -> [RecoveryAction; 4] {
    [
        RecoveryAction::RebuildSurface,
        RecoveryAction::RebuildRecipe,
        RecoveryAction::TryNextRecipe,
        RecoveryAction::UseSoftware,
    ]
}

#[test]
fn device_loss_reaches_software_window_present_and_resets_recovery() {
    let mut run = run_recovery(None);

    assert_eq!(run.status, 0);
    assert_eq!(
        run.actions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        expected_recovery_actions()
    );
    assert_eq!(run.metrics.get().present_calls, 1);
    assert_eq!(run.window.presenter.present_count(), 1);
    assert_eq!(
        run.window.presenter.state.last_pixels.len(),
        (WIDTH * HEIGHT) as usize
    );
    assert_eq!(
        run.window.presenter.state.present_calls[0].damage,
        PresentDamage::Full
    );
    assert_eq!(run.session.loop_state(), WindowLoopState::DeepIdle);

    let (_, engine) = run.session.tree_and_engine_mut();
    assert!(engine.capabilities().uses_external_presenter());
    engine.external_present_failed(Error::new(
        Errc::GraphicsDeviceLost,
        "injected next failure episode",
    ));
    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
    ));
    assert_eq!(
        run.actions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .last(),
        Some(&RecoveryAction::RebuildSurface)
    );
}

#[test]
fn failed_software_present_enters_absorbing_terminal_state_without_rebuild_loop() {
    let mut run = run_recovery(Some(Error::new(
        Errc::GraphicsDeviceLost,
        "injected Software presenter failure",
    )));

    assert_eq!(run.status, 0);
    assert_eq!(
        run.actions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .as_slice(),
        expected_recovery_actions()
    );
    assert_eq!(run.metrics.get().present_calls, 0);
    assert_eq!(run.window.presenter.present_count(), 1);
    assert_eq!(run.session.loop_state(), WindowLoopState::DeepIdle);
    assert_eq!(run.platform.event_source.state.dispatch_blocking_calls, 1);

    let action_count = run
        .actions
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .len();
    let (_, engine) = run.session.tree_and_engine_mut();
    assert!(engine.has_terminal_failure());
    assert!(matches!(
        engine.begin_frame(UpdateStrategy::FullRedraw),
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(_))
    ));
    assert_eq!(
        run.actions
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len(),
        action_count
    );
}
