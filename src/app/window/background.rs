//! 无原生窗口的生产 UI 循环；复用 WindowSession、WindowDriver 和真实 CPU 场景绘制。

use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc::SyncSender,
};
use std::time::Instant;

use crate::app::agent_workspace::AgentWorkspace;
use crate::app::application::app_handle::prepare_app_root;
use crate::app::queues::{app_timer::AppTimerQueue, main_thread_queue::MainThreadQueue};
use crate::app::session_runtime::AppRuntime;
use crate::app::window::offscreen_window::{OffscreenWindow, WorkspaceClipboard};
use crate::app::window::window_driver::{WindowDriver, WindowFrameContext};
use crate::app::window::window_session::WindowSession;
use crate::core::{Errc, Error, Point, Result, WindowId};
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::draw::{FontService, ImageService, RenderTarget, Renderer};
use crate::platform::windowing::event::EventLoopWaker;
use crate::ui::{AppState, widget_runtime::clipboard};

pub(crate) type Ready = Result<(PathBuf, String)>;

pub(crate) fn run(
    config: AgentWorkspace,
    runtime: &AppRuntime,
    stop: &AtomicBool,
    ready: SyncSender<Ready>,
) -> Result<()> {
    let result = run_inner(config, runtime, stop, &ready);
    if let Err(error) = &result {
        let _ = ready.try_send(Err(error.clone()));
        runtime.diagnostics().report_with_origin(
            error.clone(),
            crate::diagnostics::ReportOrigin::framework("agent_workspace", "run"),
        );
    }
    result
}

fn run_inner(
    config: AgentWorkspace,
    runtime: &AppRuntime,
    stop: &AtomicBool,
    ready: &SyncSender<Ready>,
) -> Result<()> {
    let id = WindowId::ROOT;
    let owner = std::thread::current();
    let waker = EventLoopWaker::new(move || owner.unpark());
    runtime.set_event_loop_waker(waker.clone());
    let state = AppState::new();
    state.set_event_loop_waker(waker);
    let timers = AppTimerQueue::new();
    let tasks = MainThreadQueue::new();
    let alive = Arc::new(AtomicBool::new(true));
    let signal = GraphicsFaultSignal::default();
    runtime.register_session_with_graphics_faults(
        id,
        timers.clone(),
        tasks.clone(),
        alive,
        signal.clone(),
    );
    runtime.enable_agent_control();

    let mut fonts = FontService::new();
    let bundle = config.fonts.unwrap_or_else(|| {
        // CPU 场景呈现需要真实字形资源；位图测量后备不能冒充可绘制字体。
        // 默认正文使用仓库已有 OFL 字体，避免访问桌面或先装 Lucide 污染正文索引。
        crate::draw::FontBundle::from_static(
            "Noto Sans CJK SC",
            include_bytes!("../../../assets/fonts/NotoSansCJKsc-Regular.otf"),
        )
    });
    fonts.install_font_bundle(&bundle)?;
    crate::ui::widgets::icon::init_static_lucide_font(
        include_bytes!("../../../assets/fonts/lucide.ttf"),
        &mut fonts,
    );
    // 后台操作面同样让布局与 CPU 绘制共享本线程的字体权威。
    let fonts = std::rc::Rc::new(fonts);
    let _layout_fonts = crate::ui::widget_runtime::measurement::LayoutFontScope::enter(
        std::rc::Rc::clone(&fonts),
    );
    let images = ImageService::new();
    let theme = RefCell::new(config.theme);
    let diagnostics = runtime.diagnostics();
    let mut renderer = Renderer::cpu();
    renderer.initialize(config.width, config.height)?;
    let mut window = OffscreenWindow::new(id, config.width, config.height, signal)?;
    let root = config.root;
    let mut session = WindowSession::from_root_factory_for_window(
        id,
        move || prepare_app_root(root(), None, id),
        Box::new(renderer),
        config.width,
        config.height,
    );
    session.set_window_focused(false);
    session.set_app_state(state);
    session.set_app_timers(timers);
    session.set_main_thread_queue(tasks);
    if let Some(queue) = runtime.agent_command_queue(id) {
        session.set_agent_command_queue(queue);
    }
    session.set_agent_command_executor(runtime.isolated_agent_command_executor());
    session.set_agent_confirm_ui(config.confirmation);
    let registration = runtime
        .register_agent_window(
            id,
            config.title.clone(),
            false,
            true,
            false,
            config.width,
            config.height,
            false,
            false,
            false,
        )
        .ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "background workspace registration failed",
            )
        })?;
    if !session.bind_agent_window(registration) {
        return Err(Error::new(
            Errc::InvalidState,
            "background workspace semantic binding failed",
        ));
    }
    session.parts_mut().tree.bind_invalidation();
    session.parts_mut().tree.mark_full_frame_dirty();
    let mut driver = WindowDriver::new(config.width, config.height, false, diagnostics.clone());
    let cursor = Cell::new(Point::new(0.0, 0.0));
    let mut private_clipboard = WorkspaceClipboard::default();
    let mut announced = false;

    while !stop.load(Ordering::Acquire) && !runtime.is_shutting_down() && !window.closed {
        let parts = session.parts_mut();
        let now = Instant::now();
        let work = driver.has_frame_work(
            now,
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            &parts.main_thread_queue,
            parts.agent_commands,
            parts.pending_root,
            parts.reconcile_pending,
        );
        if work {
            clipboard::with_clipboard(&mut private_clipboard, || {
                driver.drive_frame(WindowFrameContext {
                    tree: parts.tree,
                    engine: parts.engine,
                    active_work: parts.active_work,
                    app_timers: &parts.app_timers,
                    main_thread_queue: &parts.main_thread_queue,
                    agent_commands: parts.agent_commands,
                    view_factory: Some(parts.view_factory),
                    pending_root: parts.pending_root,
                    reconcile_pending: parts.reconcile_pending,
                    loop_state: parts.loop_state,
                    text_input: parts.text_input,
                    semantic_state: parts.semantic_state,
                    platform_window: &mut window,
                    platform: None,
                    font_service: &fonts,
                    image_service: &images,
                    theme: &theme,
                    debug_mode: &diagnostics,
                    debug_correlation_id: None,
                    hud_state: None,
                    cursor_pos: &cursor,
                    metrics: None,
                    now,
                    had_events: false,
                    had_layout_event: false,
                    next_external_deadline: None,
                    on_runtime_tasks: &mut |_, _| {},
                    on_frame: &|_, _, _| {},
                });
            });
            // 非命令回调排入原生动作也是契约失败，不能交给前台或静默视为完成。
            if !parts.tree.take_window_actions().is_empty() {
                return Err(Error::new(
                    Errc::InvalidState,
                    "background callback requested desktop window operations",
                ));
            }
            if parts.tree.is_fail_stopped() {
                return Err(Error::new(
                    Errc::InvalidState,
                    "background workspace UI stopped",
                ));
            }
            if !announced
                && parts
                    .semantic_state
                    .snapshot()
                    .is_some_and(|s| s.presented_revision > 0)
            {
                let info = runtime
                    .start_agent_transport(&config.title)
                    .map_err(|error| {
                        Error::new(
                            Errc::IoError,
                            format!("background agent transport startup failed: {error}"),
                        )
                    })?;
                ready
                    .send(Ok((info.discovery_path, info.endpoint)))
                    .map_err(|_| {
                        Error::new(
                            Errc::InvalidState,
                            "background workspace startup was cancelled",
                        )
                    })?;
                announced = true;
            }
            continue;
        }
        let deadline = driver.next_deadline(
            now,
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            &parts.main_thread_queue,
            parts.agent_commands,
            parts.pending_root,
            *parts.reconcile_pending,
        );
        // unpark 保留令牌，检查工作与停车之间到达的命令不会丢失；空闲时不轮询桌面或语义树。
        match deadline {
            Some(deadline) => {
                std::thread::park_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => std::thread::park(),
        }
    }
    session.try_shutdown()?;
    runtime.close_session(id);
    Ok(())
}
