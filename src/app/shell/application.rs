//! 应用入口 — 统一 GUI / CLI 生命周期。

use std::cell::{Cell, RefCell};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};

use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::app_handle::AppHandle;
use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::event_loop::run_window_session_loop_with_system_theme_and_tasks;
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::session_runtime::{AppRuntime, OpenWindowRequest};
use crate::app::shell::cli::Cli;
use crate::app::shell::di::Container;
use crate::app::test_clock::{system_clock, AppClock};
use crate::app::window_config::WindowConfig;
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::core::{Point, WindowId};
use crate::data::SettingsService;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::ThemeSnapshot;
use crate::draw::pipeline::{FrameRenderInput, FrameRenderer, InvalidationSource};
use crate::draw::traits::GraphicsEngine;
use crate::draw::{GpuEngine, RenderOutcome, SoftwareEngine};
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::platform::Platform;
use crate::native::traits::window::PlatformWindow;
use crate::native::{create_gpu_context, create_platform};
use crate::ui::theme::{DesignTokens, DynTokens, Theme};
use crate::ui::traits::TokenProvider;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::{AppState, SystemEvent, WidgetCore, WidgetTree};

// ════════════════════════════════════════════════════════════════════════════
// 应用模式
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    GUI,
    CLI,
}

struct SecondaryWindowSession {
    _window: Box<dyn PlatformWindow>,
    session: WindowSession,
    handle: AppHandle,
    frame_renderer: FrameRenderer,
    rendered_first: bool,
    last_frame: Option<Instant>,
    window_visible: bool,
    initial_size: (i32, i32),
}

impl SecondaryWindowSession {
    fn window_id(&self) -> WindowId {
        self.session.window_id()
    }

    fn handle_event(&mut self, platform: &mut dyn Platform, event: &UiEvent) -> bool {
        if event.type_ == UiEventType::WindowClose {
            self.handle.mark_closed();
            return false;
        }

        let parts = self.session.parts_mut();
        match event.type_ {
            UiEventType::WindowResize => {
                if let UiEventPayload::Resize(ref d) = event.payload {
                    if d.width > 0 && d.height > 0 {
                        parts.engine.resize(d.width, d.height);
                        self._window.resize_notify(d.width, d.height);
                        self.initial_size = (d.width, d.height);
                        self.window_visible = true;
                    }
                }
            }
            UiEventType::WindowMaximize => {
                let info = platform.display().info(0);
                let w = info.bounds.w as i32;
                let h = info.bounds.h as i32;
                if w > 0 && h > 0 {
                    parts.engine.resize(w, h);
                    self._window.resize_notify(w, h);
                    self.window_visible = true;
                }
            }
            UiEventType::WindowRestore => {
                let (w, h) = self.initial_size;
                parts.engine.resize(w, h);
                self._window.resize_notify(w, h);
                self.window_visible = true;
            }
            UiEventType::WindowMinimize => {
                self.window_visible = false;
            }
            _ => {}
        }

        if let Some(system_event) = map_ui_event(event) {
            parts.tree.dispatch_event(&system_event);
        }
        platform.event_bus().publish(event);
        true
    }

    fn drain_main_thread_work(&mut self) -> bool {
        let parts = self.session.parts_mut();
        let mut main_thread_context =
            MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        let had_main_thread_work = parts.main_thread_queue.drain(&mut main_thread_context);

        if *parts.reconcile_pending {
            let root = parts
                .pending_root
                .take()
                .or_else(|| parts.view_factory.build());
            if let Some(root) = root {
                ViewAdapter::reconcile_nodes(parts.tree, root);
            }
            *parts.reconcile_pending = false;
            return true;
        }

        had_main_thread_work
    }

    fn drain_frame(
        &mut self,
        font_service: &FontService,
        image_service: &ImageService,
        theme: &RefCell<Theme>,
        debug_mode: &Cell<bool>,
        cursor_pos: &Cell<Point>,
        clock: &dyn AppClock,
    ) -> bool {
        let now = clock.now();
        let last_frame = self.last_frame.get_or_insert(now);
        let parts = self.session.parts_mut();

        parts
            .active_work
            .sync_app_timers(parts.app_timers.deadlines());
        let due_work = parts.active_work.drain_due(now);
        let had_registered_work = !due_work.is_empty();
        dispatch_due_secondary_active_work(parts.tree, &parts.app_timers, &due_work, clock);

        let mut main_thread_context =
            MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        let had_main_thread_work = parts.main_thread_queue.drain(&mut main_thread_context);
        parts
            .active_work
            .sync_timers(parts.tree.active_timers(), clock.now());
        parts
            .active_work
            .sync_app_timers(parts.app_timers.deadlines());

        if parts.tree.take_reconcile_requested() {
            *parts.reconcile_pending = true;
        }

        let pending_layout_work = parts
            .tree
            .invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_layout();
        let active_frame = had_registered_work
            || had_main_thread_work
            || *parts.reconcile_pending
            || !self.rendered_first
            || pending_layout_work
            || parts.tree.has_render_work();

        if active_frame {
            let dt = (now - *last_frame).as_secs_f64().min(0.05);
            *last_frame = now;
            let animating = parts.tree.update(dt);
            sync_secondary_animation_deadline(parts.tree, parts.active_work, animating, now);
            let _effects_ran = parts.tree.tick_effects();
        }

        if parts.tree.take_reconcile_requested() {
            *parts.reconcile_pending = true;
        }

        if *parts.reconcile_pending {
            let root = parts
                .pending_root
                .take()
                .or_else(|| parts.view_factory.build());
            if let Some(root) = root {
                ViewAdapter::reconcile_nodes(parts.tree, root);
            }
            *parts.reconcile_pending = false;
        }

        let has_layout_work = parts
            .tree
            .invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_layout();
        if self.window_visible && (!self.rendered_first || has_layout_work) {
            let before_version = parts.tree.tree_version();
            parts.tree.layout();
            sync_secondary_root_frame_to_engine(parts.tree, parts.engine);
            if parts.tree.tree_version() != before_version {
                parts.tree.layout();
                parts.tree.mark_full_frame_dirty();
            }
        }

        let need_render =
            self.window_visible && (!self.rendered_first || parts.tree.has_render_work());
        let engine_capabilities = parts.engine.capabilities();

        if !self.rendered_first && need_render {
            parts.tree.mark_full_frame_dirty();
            parts.tree.layout();
        }

        let dirty_region = parts.tree.dirty_region();
        let (outcome, outcome_source) = if !need_render {
            (RenderOutcome::Idle, InvalidationSource::None)
        } else {
            let theme_ref = theme.borrow();
            let snapshot = ThemeSnapshot::new(theme_ref.tokens());
            let scroll_move = parts.tree.drain_scroll_region_move();
            let hover_pos = if debug_mode.get() {
                Some(cursor_pos.get())
            } else {
                None
            };
            let frame_out = self.frame_renderer.render_frame(
                parts.engine,
                parts.tree,
                FrameRenderInput {
                    rendered_first: self.rendered_first,
                    dirty_region: &dirty_region,
                    tree_version: parts.tree.tree_version(),
                    scroll_move,
                    theme: snapshot,
                    font: font_service.loaded_font_handle,
                    font_service,
                    image_service,
                    debug_mode: debug_mode.get(),
                    hover_pos,
                    metrics: None,
                },
            );
            self.rendered_first = true;
            (frame_out.outcome, frame_out.inv_source)
        };

        parts.tree.reset_dirty();

        if let RenderOutcome::Present(damage) = outcome {
            let _ = outcome_source;
            if engine_capabilities.uses_external_presenter() {
                let canvas = parts.engine.canvas_2d();
                let cw = canvas.width();
                let ch = canvas.height();
                if let Err(e) = self._window.presenter().present(
                    canvas.pixels_mut(),
                    cw,
                    ch,
                    damage.to_present_damage(),
                ) {
                    crate::core::log::error_fn(format!(
                        "[Application] secondary present failed: {}",
                        e.short_what()
                    ));
                }
            }
        }

        *parts.loop_state = secondary_next_loop_state(parts.tree, parts.active_work);
        active_frame || need_render
    }

    fn next_deadline(&mut self) -> Option<Instant> {
        let parts = self.session.parts_mut();
        parts
            .active_work
            .sync_app_timers(parts.app_timers.deadlines());
        parts.active_work.next_deadline()
    }

    fn close(self) {
        self.handle.mark_closed();
    }
}

// ════════════════════════════════════════════════════════════════════════════
// App — 统一应用入口
// ════════════════════════════════════════════════════════════════════════════

/// 应用入口：GUI（View 根节点）或 CLI 模式。
pub struct App {
    mode: AppMode,
    title: String,
    size: (i32, i32),
    theme: Theme,
    follow_system_theme: bool,
    app_state: AppState,
    app_timers: AppTimerQueue,
    main_thread_queue: MainThreadQueue,
    runtime: AppRuntime,
    handle_alive: Arc<AtomicBool>,
    root_factory: Option<Arc<dyn Fn() -> ViewNode + Send + Sync>>,
    on_start: Option<Box<dyn FnOnce(AppHandle) + Send>>,
    on_window_start: Option<Arc<dyn Fn(AppHandle) + Send + Sync>>,
    on_exit: Option<Box<dyn Fn(&UiEvent) -> bool>>,
    cli: Option<Cli>,
    container: Container,
    settings_path: Option<String>,
    exit_code: i32,
}

impl Default for App {
    fn default() -> Self {
        let app_timers = AppTimerQueue::new();
        let main_thread_queue = MainThreadQueue::new();
        let handle_alive = Arc::new(AtomicBool::new(true));
        let runtime = AppRuntime::new();
        runtime.register_session(
            WindowId::ROOT,
            app_timers.clone(),
            main_thread_queue.clone(),
            handle_alive.clone(),
        );

        Self {
            mode: AppMode::GUI,
            title: "UIX App".to_string(),
            size: (800, 600),
            theme: Theme::antd_light(),
            follow_system_theme: false,
            app_state: AppState::new(),
            app_timers,
            main_thread_queue,
            runtime,
            handle_alive,
            root_factory: None,
            on_start: None,
            on_window_start: None,
            on_exit: None,
            cli: None,
            container: Container::new(),
            settings_path: None,
            exit_code: 0,
        }
    }
}

impl App {
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置窗口标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置窗口初始尺寸。
    pub fn size(mut self, width: i32, height: i32) -> Self {
        self.size = (width, height);
        self
    }

    /// 设置主题（GUI + root 时生效）。
    pub fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// 设置是否在运行中跟随 OS 主题变化（默认 false）。
    pub fn follow_system_theme(mut self, follow: bool) -> Self {
        self.follow_system_theme = follow;
        self
    }

    /// 延迟一次执行 App 级回调；回调在主循环线程执行。
    pub fn run_after<F>(&self, delay: Duration, f: F) -> TimerHandle
    where
        F: FnOnce() + Send + 'static,
    {
        self.app_timers.run_after(delay, f)
    }

    /// 按固定间隔重复执行 App 级回调；`TimerHandle` drop/cancel 后停止。
    pub fn run_interval<F>(&self, interval: Duration, f: F) -> TimerHandle
    where
        F: FnMut() + Send + 'static,
    {
        self.app_timers.run_interval(interval, f)
    }

    /// 投递一次主线程回调；回调在当前窗口 session 的帧内 drain。
    pub fn post_to_ui<F>(&self, f: F)
    where
        F: FnOnce() + Send + 'static,
    {
        self.runtime.post_to_ui(WindowId::ROOT, f);
    }

    pub fn on_start<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AppHandle) + Send + 'static,
    {
        self.on_start = Some(Box::new(f));
        self
    }

    pub fn on_window_start<F>(mut self, f: F) -> Self
    where
        F: Fn(AppHandle) + Send + Sync + 'static,
    {
        self.on_window_start = Some(Arc::new(f));
        self
    }

    /// 设置根 View（GUI 模式必需）。
    pub fn root<F>(mut self, build_root: F) -> Self
    where
        F: Fn() -> ViewNode + Send + Sync + 'static,
    {
        self.root_factory = Some(Arc::new(build_root));
        self
    }

    /// 退出条件：返回 `true` 时结束事件循环。
    pub fn on_exit<F: Fn(&UiEvent) -> bool + 'static>(mut self, f: F) -> Self {
        self.on_exit = Some(Box::new(f));
        self
    }

    /// 设置应用模式。
    pub fn mode(mut self, m: AppMode) -> Self {
        self.mode = m;
        self
    }

    /// 注册 CLI 命令处理器。
    pub fn cli(mut self, cli: Cli) -> Self {
        self.cli = Some(cli);
        self
    }

    /// Opt in to loading SettingsService once before `run()` enters its mode.
    pub fn settings(mut self, path: impl Into<String>) -> Self {
        self.settings_path = Some(path.into());
        self
    }

    #[cfg(test)]
    pub(crate) fn app_handle(&self) -> AppHandle {
        self.app_handle_for_window(WindowId::ROOT)
    }

    pub(crate) fn app_handle_for_window(&self, window_id: WindowId) -> AppHandle {
        AppHandle::new(
            window_id,
            self.app_state.clone(),
            self.runtime.clone(),
            self.container.clone(),
            self.handle_alive.clone(),
        )
    }

    /// 注册全局单例。
    pub fn singleton<T: 'static + Send + Clone>(mut self, instance: T) -> Self {
        self.container.singleton(instance);
        self
    }

    // ── 查询 ──────────────────────────────────────────────────────

    pub fn current_mode(&self) -> AppMode {
        self.mode
    }

    pub fn window_title(&self) -> &str {
        &self.title
    }

    pub fn window_size(&self) -> (i32, i32) {
        self.size
    }

    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    pub fn container(&self) -> &Container {
        &self.container
    }

    pub fn app_state(&self) -> AppState {
        self.app_state.clone()
    }

    // ── 运行 ──────────────────────────────────────────────────────

    pub fn run(mut self) -> i32 {
        if !self.load_configured_settings() {
            return self.exit_code;
        }
        match self.mode {
            AppMode::CLI => self.run_cli(),
            AppMode::GUI => self.run_gui(),
        }
    }

    fn load_configured_settings(&mut self) -> bool {
        let Some(path) = self.settings_path.as_deref() else {
            return true;
        };

        let mut settings = SettingsService::new();
        if let Err(err) = settings.load(path) {
            crate::core::log::error_fn(format!("load settings failed: {}", err.short_what()));
            self.exit_code = 1;
            return false;
        }

        self.container.singleton(settings);
        true
    }

    fn run_cli(&mut self) -> i32 {
        if let Some(ref mut cli) = self.cli {
            let args = std::env::args().collect::<Vec<_>>();
            self.exit_code = cli.run(&args);
        } else {
            crate::core::log::info_fn("Running in CLI mode (no commands registered)");
        }
        self.exit_code
    }

    fn run_gui(mut self) -> i32 {
        let root_factory = match self.root_factory.take() {
            Some(factory) => factory,
            None => {
                crate::core::log::error_fn("GUI 模式须调用 .root() 设置根 View");
                return 1;
            }
        };

        let (w, h) = self.size;

        let mut platform = match create_platform() {
            Ok(p) => p,
            Err(e) => {
                crate::core::log::error_fn(format!("create_platform 失败: {:?}", e));
                return 1;
            }
        };

        let mut platform_window = match platform.window_manager().create_window(&self.title, w, h) {
            Ok(win) => win,
            Err(e) => {
                crate::core::log::error_fn(format!("create_window 失败: {:?}", e));
                return 1;
            }
        };
        platform_window.center_on_screen();
        platform_window.show();
        platform_window.raise();
        self.runtime
            .set_event_loop_waker(platform.event_loop().waker());

        let engine = match create_preferred_engine(platform_window.as_mut(), w, h) {
            Some(engine) => engine,
            None => return 1,
        };

        let mut font_service = FontService::new();
        font_service.load_default_system_font(14.0, platform.system_info());
        let image_service = ImageService::new();

        let system_theme_tokens = if self.follow_system_theme {
            let tokens = Arc::new(DynTokens::new(if platform.display().is_dark_mode() {
                DesignTokens::antd_dark()
            } else {
                DesignTokens::antd_light()
            }));
            let provider: Arc<dyn TokenProvider> = tokens.clone();
            self.theme = Theme::from_arc(provider);
            Some(tokens)
        } else {
            None
        };

        let root_window_id = platform_window.window_id();
        let mut session = WindowSession::from_root_factory_for_window(
            root_window_id,
            move || root_factory(),
            engine,
            w,
            h,
        );
        session.set_app_state(self.app_state.clone());
        session.set_app_timers(self.app_timers.clone());
        session.set_main_thread_queue(self.main_thread_queue.clone());
        self.runtime.register_session(
            root_window_id,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
        );
        let app_handle = self.app_handle_for_window(root_window_id);
        if let Some(on_start) = self.on_start.take() {
            on_start(app_handle.clone());
        }
        let secondary_windows = RefCell::new(Vec::new());
        let secondary_clock = system_clock();
        drain_pending_open_windows(
            &mut *platform,
            &self.runtime,
            &self.app_state,
            &self.container,
            self.on_window_start.as_ref(),
            &mut secondary_windows.borrow_mut(),
        );
        drain_secondary_window_queues(&mut secondary_windows.borrow_mut());

        let theme = RefCell::new(self.theme);
        let debug_mode = Cell::new(false);
        let cursor_pos = Cell::new(Point::new(0.0, 0.0));
        drain_secondary_window_frames(
            &mut secondary_windows.borrow_mut(),
            &font_service,
            &image_service,
            &theme,
            &debug_mode,
            &cursor_pos,
            secondary_clock.as_ref(),
        );
        let on_exit = self
            .on_exit
            .unwrap_or_else(|| Box::new(|_: &UiEvent| false));

        let runtime = self.runtime.clone();
        let app_state = self.app_state.clone();
        let container = self.container.clone();
        let on_window_start = self.on_window_start.clone();

        run_window_session_loop_with_system_theme_and_tasks(
            &mut *platform,
            &mut *platform_window,
            &mut session,
            &font_service,
            &image_service,
            &theme,
            system_theme_tokens.as_deref(),
            &debug_mode,
            &cursor_pos,
            None,
            map_ui_event,
            |ev| on_exit(ev),
            |platform| {
                drain_pending_open_windows(
                    platform,
                    &runtime,
                    &app_state,
                    &container,
                    on_window_start.as_ref(),
                    &mut secondary_windows.borrow_mut(),
                );
                drain_secondary_window_queues(&mut secondary_windows.borrow_mut());
                drain_secondary_window_frames(
                    &mut secondary_windows.borrow_mut(),
                    &font_service,
                    &image_service,
                    &theme,
                    &debug_mode,
                    &cursor_pos,
                    secondary_clock.as_ref(),
                );
            },
            |event, platform| {
                dispatch_secondary_window_event(
                    &mut secondary_windows.borrow_mut(),
                    platform,
                    event,
                );
            },
            || secondary_windows_next_deadline(&mut secondary_windows.borrow_mut()),
            |_, _, _| {},
        );

        let mut secondary_windows = secondary_windows.into_inner();
        for window in secondary_windows.drain(..) {
            window.close();
        }
        app_handle.mark_closed();

        0
    }
}

fn drain_pending_open_windows(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    let mut created = 0;
    while let Some(request) = runtime.take_next_open_window() {
        if let Some(mut window) =
            create_secondary_window(platform, runtime, app_state, container, request)
        {
            if let Some(callback) = on_window_start {
                callback(window.handle.clone());
            }
            window.drain_main_thread_work();
            secondary_windows.push(window);
            created += 1;
        }
    }
    created
}

fn drain_secondary_window_queues(secondary_windows: &mut [SecondaryWindowSession]) -> bool {
    let mut drained = false;
    for window in secondary_windows {
        drained |= window.drain_main_thread_work();
    }
    drained
}

fn drain_secondary_window_frames(
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    let mut drained = false;
    for window in secondary_windows {
        drained |= window.drain_frame(
            font_service,
            image_service,
            theme,
            debug_mode,
            cursor_pos,
            clock,
        );
    }
    drained
}

fn secondary_windows_next_deadline(
    secondary_windows: &mut [SecondaryWindowSession],
) -> Option<Instant> {
    secondary_windows
        .iter_mut()
        .filter_map(SecondaryWindowSession::next_deadline)
        .min()
}

fn dispatch_secondary_window_event(
    secondary_windows: &mut Vec<SecondaryWindowSession>,
    platform: &mut dyn Platform,
    event: &UiEvent,
) -> bool {
    let Some(window_id) = event.window_id else {
        return false;
    };
    let Some(index) = secondary_windows
        .iter()
        .position(|window| window.window_id() == window_id)
    else {
        return false;
    };

    if secondary_windows[index].handle_event(platform, event) {
        true
    } else {
        secondary_windows.remove(index);
        true
    }
}

fn create_secondary_window(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    request: OpenWindowRequest,
) -> Option<SecondaryWindowSession> {
    let OpenWindowRequest {
        window_id,
        config,
        app_timers,
        main_thread_queue,
        alive,
    } = request;
    let WindowConfig {
        title,
        width,
        height,
        root,
    } = config;

    let mut platform_window = match platform
        .window_manager()
        .create_window(&title, width, height)
    {
        Ok(window) => window,
        Err(e) => {
            runtime.close_session(window_id);
            crate::core::log::error_fn(format!(
                "open_window create_window failed: {}",
                e.short_what()
            ));
            return None;
        }
    };
    if platform_window.window_id() != window_id {
        let actual = platform_window.window_id();
        platform_window.close();
        runtime.close_session(window_id);
        crate::core::log::error_fn(format!(
            "open_window window_id mismatch: reserved={}, native={}",
            window_id.raw(),
            actual.raw()
        ));
        return None;
    }

    platform_window.center_on_screen();
    platform_window.show();
    platform_window.raise();

    let engine = match create_preferred_engine(platform_window.as_mut(), width, height) {
        Some(engine) => engine,
        None => {
            platform_window.close();
            runtime.close_session(window_id);
            return None;
        }
    };

    let mut session = WindowSession::from_root_factory_for_window(
        window_id,
        move || root(),
        engine,
        width,
        height,
    );
    session.set_app_state(app_state.clone());
    session.set_app_timers(app_timers);
    session.set_main_thread_queue(main_thread_queue);
    let handle = AppHandle::new(
        window_id,
        app_state.clone(),
        runtime.clone(),
        container.clone(),
        alive,
    );

    Some(SecondaryWindowSession {
        _window: platform_window,
        session,
        handle,
        frame_renderer: FrameRenderer::new(),
        rendered_first: false,
        last_frame: None,
        window_visible: true,
        initial_size: (width, height),
    })
}

fn dispatch_due_secondary_active_work(
    tree: &mut WidgetTree,
    app_timers: &AppTimerQueue,
    due_work: &[ActiveWorkKind],
    clock: &dyn AppClock,
) {
    for work in due_work {
        match *work {
            ActiveWorkKind::Timer(id) => {
                if let Ok(id) = u32::try_from(id) {
                    let _ = tree.dispatch_event(&SystemEvent::Timer { id });
                }
            }
            ActiveWorkKind::AppTimer(id) => {
                app_timers.fire(id, clock.now());
            }
            _ => {}
        }
    }
}

fn sync_secondary_animation_deadline(
    tree: &WidgetTree,
    active_work: &mut ActiveWorkRegistry,
    animating: bool,
    now: Instant,
) {
    let Some(root_id) = tree.root_id() else {
        return;
    };
    let kind = ActiveWorkKind::Animation(root_id);
    if animating {
        active_work.register(kind, now + Duration::from_millis(16));
    } else {
        active_work.unregister(kind);
    }
}

fn sync_secondary_root_frame_to_engine(tree: &mut WidgetTree, engine: &mut dyn GraphicsEngine) {
    let need_sync = tree
        .root_id()
        .and_then(|rid| tree.get(rid))
        .is_some_and(|root| {
            let ew = engine.canvas_2d().width() as f32;
            let eh = engine.canvas_2d().height() as f32;
            let rf = root.frame();
            (rf.w - ew).abs() > 0.5 || (rf.h - eh).abs() > 0.5
        });
    if need_sync {
        if let Some(rid) = tree.root_id() {
            if let Some(root_mut) = tree.get_mut(rid) {
                root_mut.set_frame(crate::core::Rect::new(
                    0.0,
                    0.0,
                    engine.canvas_2d().width() as f32,
                    engine.canvas_2d().height() as f32,
                ));
            }
        }
        tree.mark_full_frame_dirty();
        tree.layout();
    }
}

fn secondary_next_loop_state(
    tree: &WidgetTree,
    active_work: &ActiveWorkRegistry,
) -> WindowLoopState {
    let has_layout_work = tree
        .invalidation
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .has_layout();
    if tree.has_render_work() || has_layout_work {
        WindowLoopState::Active
    } else if !active_work.is_empty() {
        WindowLoopState::RegisteredActive
    } else {
        WindowLoopState::DeepIdle
    }
}

fn create_preferred_engine(
    platform_window: &mut dyn PlatformWindow,
    width: i32,
    height: i32,
) -> Option<Box<dyn GraphicsEngine>> {
    let surface = platform_window.native_surface_ptr();
    match create_gpu_context(surface, width, height).and_then(GpuEngine::new) {
        Ok(mut engine) => match engine.initialize(width, height) {
            Ok(()) => {
                crate::core::log::info_fn("GPU engine initialized");
                return Some(Box::new(engine));
            }
            Err(e) => {
                engine.shutdown();
                crate::core::log::warn_fn(format!(
                    "GPU engine initialize failed, falling back to CPU: {}",
                    e.short_what()
                ));
            }
        },
        Err(e) => {
            crate::core::log::warn_fn(format!(
                "GPU engine unavailable, falling back to CPU: {}",
                e.short_what()
            ));
        }
    }

    let mut engine = SoftwareEngine::new();
    match engine.initialize(width, height) {
        Ok(()) => {
            crate::core::log::info_fn("CPU software engine initialized");
            Some(Box::new(engine))
        }
        Err(e) => {
            crate::core::log::error_fn(format!("SoftwareEngine 初始化失败: {}", e.short_what()));
            None
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent → SystemEvent 映射（唯一实现）
// ════════════════════════════════════════════════════════════════════════════

/// 将平台 `UiEvent` 转换为 `SystemEvent`。
pub fn map_ui_event(ev: &UiEvent) -> Option<SystemEvent> {
    match ev.type_ {
        UiEventType::PointerDown => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerDown {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerUp => {
            if let UiEventPayload::PointerButton(ref d) = ev.payload {
                Some(SystemEvent::PointerUp {
                    pos: d.pos,
                    button: d.btn,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::PointerMove => {
            if let UiEventPayload::PointerMove(ref d) = ev.payload {
                Some(SystemEvent::PointerMove {
                    pos: d.pos,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::Wheel => {
            if let UiEventPayload::Wheel(ref d) = ev.payload {
                Some(SystemEvent::Wheel {
                    pos: d.pos,
                    delta: Point::new(d.delta_x, d.delta_y),
                })
            } else {
                None
            }
        }
        UiEventType::KeyDown => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(SystemEvent::KeyDown {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::KeyUp => {
            if let UiEventPayload::Key(ref d) = ev.payload {
                Some(SystemEvent::KeyUp {
                    key: d.key,
                    mods: d.mods,
                })
            } else {
                None
            }
        }
        UiEventType::Copy => Some(SystemEvent::Copy),
        UiEventType::Cut => Some(SystemEvent::Cut),
        UiEventType::Paste => {
            if let UiEventPayload::Clipboard(ref d) = ev.payload {
                Some(SystemEvent::Paste {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::TextInput => {
            if let UiEventPayload::TextInput(ref d) = ev.payload {
                Some(SystemEvent::TextInput {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ImeCompositionStart => Some(SystemEvent::ImeCompositionStart),
        UiEventType::ImeCompositionUpdate => {
            if let UiEventPayload::ImeComposition(ref d) = ev.payload {
                Some(SystemEvent::ImeCompositionUpdate {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ImeCompositionEnd => {
            if let UiEventPayload::ImeComposition(ref d) = ev.payload {
                Some(SystemEvent::ImeCompositionEnd {
                    text: d.text.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::ThemeChanged => {
            if let UiEventPayload::ThemeChanged(ref d) = ev.payload {
                Some(SystemEvent::ThemeChanged { is_dark: d.is_dark })
            } else {
                None
            }
        }
        UiEventType::LocaleChanged => {
            if let UiEventPayload::LocaleChanged(ref d) = ev.payload {
                Some(SystemEvent::LocaleChanged {
                    locale: d.locale.clone(),
                })
            } else {
                None
            }
        }
        UiEventType::WindowResize => {
            if let UiEventPayload::Resize(ref d) = ev.payload {
                Some(SystemEvent::Resize {
                    width: d.width as f32,
                    height: d.height as f32,
                })
            } else {
                None
            }
        }
        UiEventType::WindowMaximize => Some(SystemEvent::WindowMaximize),
        UiEventType::WindowMinimize => Some(SystemEvent::WindowMinimize),
        UiEventType::WindowRestore => Some(SystemEvent::WindowRestore),
        UiEventType::WindowFocus => Some(SystemEvent::WindowFocus),
        UiEventType::WindowBlur => Some(SystemEvent::WindowBlur),
        UiEventType::Timer => {
            if let UiEventPayload::Timer(ref d) = ev.payload {
                Some(SystemEvent::Timer { id: d.timer_id })
            } else {
                None
            }
        }
        UiEventType::FileDrop => {
            if let UiEventPayload::FileDrop(ref d) = ev.payload {
                Some(SystemEvent::FileDrop {
                    files: d.files.clone(),
                    position: d.position,
                })
            } else {
                None
            }
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "../../tests/app/shell/application.rs"]
mod tests;
