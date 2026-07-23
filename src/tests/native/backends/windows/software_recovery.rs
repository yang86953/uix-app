use crate::app::clock::AppClock;
use crate::app::event_loop::event_loop::run_window_session_loop_with_clock;
use crate::app::shell::application::graphics_recovery_rebuilder;
use crate::app::window_session::WindowSession;
use crate::draw::renderer::bootstrap::bootstrap_renderer;
use crate::draw::renderer::RenderMetrics;
use crate::draw::renderer::{GraphicsFailure, RecoveryDriver};
use crate::draw::{Canvas2D, GraphicsCapabilities, UpdateStrategy};
use crate::native::backends::windows::consts::WM_CLOSE;
use crate::native::backends::windows::ffi::PostMessageW;
use crate::native::graphics::vulkan::platform::context::accept_device_wait_for_shutdown;
use crate::native::traits::present::NativeSurfaceHandle;
use crate::tests::common::*;
use crate::ui::view::ViewNode;
use crate::ui::widgets::Container;
use ash::vk;
use std::sync::atomic::{AtomicUsize, Ordering};

const WIDTH: i32 = 160;
const HEIGHT: i32 = 120;

struct InjectedDeviceLossEngine {
    inner: Box<dyn RenderTarget>,
    vulkan_lost_wait_teardowns: Option<Arc<AtomicUsize>>,
}

impl InjectedDeviceLossEngine {
    fn new(
        inner: Box<dyn RenderTarget>,
        vulkan_lost_wait_teardowns: Option<Arc<AtomicUsize>>,
    ) -> Self {
        Self {
            inner,
            vulkan_lost_wait_teardowns,
        }
    }
}

impl RenderTarget for InjectedDeviceLossEngine {
    fn initialize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.initialize(width, height)
    }

    fn try_shutdown(&mut self) -> Result<(), Error> {
        if let Some(teardowns) = &self.vulkan_lost_wait_teardowns {
            accept_device_wait_for_shutdown(Err(vk::Result::ERROR_DEVICE_LOST))?;
            teardowns.fetch_add(1, Ordering::Relaxed);
        }
        self.inner.try_shutdown()
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.inner.resize(width, height)
    }

    fn begin_frame(&mut self, _strategy: UpdateStrategy) -> RenderOutcome {
        RenderOutcome::Failed(GraphicsFailure::DeviceLost(Error::new(
            Errc::GraphicsDeviceLost,
            "injected Windows device loss",
        )))
    }

    fn end_frame(&mut self, damage: &DamageRegion) -> RenderOutcome {
        self.inner.end_frame(damage)
    }

    fn canvas_2d(&mut self) -> &mut dyn Canvas2D {
        self.inner.canvas_2d()
    }

    fn capabilities(&self) -> GraphicsCapabilities {
        self.inner.capabilities()
    }

    fn dpi(&self) -> f32 {
        self.inner.dpi()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.inner.device_pixel_ratio()
    }

    fn orientation(&self) -> crate::draw::geometry::spatial::Orientation {
        self.inner.orientation()
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

#[derive(Debug, Default)]
struct RecoveryTrace {
    actions: Vec<RecoveryAction>,
    successful_native_rebuilds: Vec<RecoveryAction>,
    next_recipe_succeeded: bool,
}

#[test]
#[ignore = "需要 Windows 交互式桌面与至少两个可用原生 GPU recipe"]
fn device_loss_crosses_real_windows_recipes_and_commits_software_frame() {
    let mut platform = crate::native::create_platform().expect("Windows platform");
    let mut window = platform
        .window_manager()
        .create_window("UIX graphics recovery probe", WIDTH, HEIGHT)
        .expect("Windows probe window");
    window.show().expect("show Windows probe window");

    let hwnd = window.native_surface_ptr();
    // SAFETY: 窗口在整个同步 bootstrap、恢复与事件循环期间存活，且操作始终留在当前线程。
    let surface = unsafe { NativeSurfaceHandle::from_raw(hwnd) };
    let bootstrap = bootstrap_renderer(surface, WIDTH, HEIGHT, GraphicsBackend::Auto)
        .unwrap_or_else(|report| panic!("native GPU bootstrap failed: {:?}", report.failures));
    assert_eq!(bootstrap.selected_recipe.backend, GraphicsBackend::Vulkan);

    let trace = Arc::new(Mutex::new(RecoveryTrace::default()));
    let recorded_trace = Arc::clone(&trace);
    let vulkan_lost_wait_teardowns = Arc::new(AtomicUsize::new(0));
    let initial_lost_wait_teardowns = Arc::clone(&vulkan_lost_wait_teardowns);
    let rebuilt_lost_wait_teardowns = Arc::clone(&vulkan_lost_wait_teardowns);
    let mut native_rebuilder =
        graphics_recovery_rebuilder(surface, GraphicsBackend::Auto, bootstrap.selected_recipe);
    let recovering = RecoveryDriver::new(
        Box::new(InjectedDeviceLossEngine::new(
            Box::new(bootstrap.renderer),
            Some(initial_lost_wait_teardowns),
        )),
        Box::new(move |action, width, height| {
            recorded_trace
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .actions
                .push(action);
            let replacement = native_rebuilder(action, width, height)?;
            if action == RecoveryAction::UseSoftware {
                return Ok(replacement);
            }

            let mut trace = recorded_trace
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            trace.successful_native_rebuilds.push(action);
            trace.next_recipe_succeeded |= action == RecoveryAction::TryNextRecipe;
            let lost_wait_teardowns = matches!(
                action,
                RecoveryAction::RebuildSurface | RecoveryAction::RebuildRecipe
            )
            .then(|| Arc::clone(&rebuilt_lost_wait_teardowns));
            Ok(Box::new(InjectedDeviceLossEngine::new(
                replacement,
                lost_wait_teardowns,
            )))
        }),
    )
    .with_extent(WIDTH, HEIGHT);

    let window_id = window.window_id();
    let mut session = WindowSession::from_root_for_window(
        window_id,
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
    let close_posted = Cell::new(false);
    let callback_after_software = Cell::new(false);
    let close_trace = Arc::clone(&trace);

    let status = run_window_session_loop_with_clock(
        platform.as_mut(),
        window.as_mut(),
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
        |_, _, _| {
            let action_count = close_trace
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .actions
                .len();
            if action_count == 4 {
                callback_after_software.set(true);
            }
            if action_count == 3 && !close_posted.replace(true) {
                // SAFETY: hwnd 仍由当前线程上的 window 持有；这里只投递异步关闭消息。
                assert_ne!(unsafe { PostMessageW(hwnd, WM_CLOSE, 0, 0) }, 0);
            }
        },
    );

    assert_eq!(status, 0);
    assert!(close_posted.get());
    assert!(!callback_after_software.get());
    assert_eq!(metrics.get().present_calls, 1);
    let trace = trace.lock().unwrap_or_else(|error| error.into_inner());
    assert_eq!(
        trace.actions,
        [
            RecoveryAction::RebuildSurface,
            RecoveryAction::RebuildRecipe,
            RecoveryAction::TryNextRecipe,
            RecoveryAction::UseSoftware,
        ]
    );
    assert!(trace
        .successful_native_rebuilds
        .contains(&RecoveryAction::TryNextRecipe));
    assert!(trace.next_recipe_succeeded);
    drop(trace);
    assert_eq!(vulkan_lost_wait_teardowns.load(Ordering::Relaxed), 3);

    let (_, engine) = session.tree_and_engine_mut();
    assert!(engine.capabilities().uses_external_presenter());
    assert!(!engine.has_terminal_failure());
    session.try_shutdown().expect("shutdown recovery session");
    window.close().expect("close Windows probe window");
}
