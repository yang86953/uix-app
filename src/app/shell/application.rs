//! 应用入口 — 统一 GUI / CLI 生命周期。

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};

use crate::app::app_handle::{
    wrap_root_with_notification_overlay, AppHandle, AppNotificationState,
};
use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::clock::{system_clock, AppClock};
use crate::app::event_loop::run_window_session_loop_with_system_theme_and_tasks;
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::session_runtime::{AppRuntime, OpenWindowRequest};
use crate::app::shell::cli::Cli;
use crate::app::shell::di::Container;
use crate::app::text_input::sync_window_text_input;
use crate::app::window_actions::{apply_pending_window_actions, configure_custom_title_bar};
use crate::app::window_config::WindowConfig;
use crate::app::window_driver::{WindowDriver, WindowFrameContext};
use crate::app::window_session::WindowSession;
use crate::core::{Errc, Error, Point, WindowId};
use crate::data::SettingsService;
use crate::draw::engine::bootstrap::{
    assemble_graphics_engine, bootstrap_graphics_engine, ProbeReport,
};
use crate::draw::engine::{GraphicsEngineRebuilder, RecoveringGraphicsEngine, RecoveryAction};
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::traits::GraphicsEngine;
use crate::draw::SoftwareEngine;
use crate::native::create_platform;
use crate::native::factory::{
    gpu_recipe_candidates, graphics_runtime_platform, try_create_gpu_recipe, GraphicsRecipe,
};
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::{GraphicsBackend, NativeSurfaceHandle};
use crate::native::traits::window::{PlatformWindow, WindowOcclusionState};
use crate::ui::theme::{DesignTokens, DynTokens, Theme};
use crate::ui::traits::TokenProvider;
use crate::ui::view::ViewNode;
use crate::ui::{AppState, SystemEvent, WidgetTree};

// ════════════════════════════════════════════════════════════════════════════
// 应用模式
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AppMode {
    #[default]
    GUI,
    CLI,
}

const GRAPHICS_BACKEND_ENV: &str = "UIX_GRAPHICS_BACKEND";
const GRAPHICS_BACKEND_SETTING_KEYS: [&str; 2] = ["graphics_backend", "uix.graphics_backend"];

fn report_window_operation_error(context: &str, result: crate::core::Result<()>) {
    if let Err(error) = result {
        crate::core::log::warn_fn(format!("{context}: {}", error.short_what()));
    }
}

fn initially_agent_presentable(window: &dyn PlatformWindow) -> bool {
    let properties = window.properties();
    properties.width() > 0
        && properties.height() > 0
        && !properties.is_minimized()
        && window.occlusion_state() != WindowOcclusionState::Occluded
}

pub(crate) struct SecondaryWindowSession {
    // session 必须先于原生窗口析构，确保 engine/GL 资源先释放。
    pub(crate) session: WindowSession,
    _window: Box<dyn PlatformWindow>,
    pub(crate) handle: AppHandle,
    driver: WindowDriver,
    pub(crate) last_frame: Option<Instant>,
}

impl SecondaryWindowSession {
    fn window_id(&self) -> WindowId {
        self.session.window_id()
    }

    fn handle_event(&mut self, platform: &mut dyn Platform, event: &UiEvent) -> bool {
        let window_id = self.window_id();
        let native_window = self._window.native_handle().native_window();
        if event.type_ == UiEventType::WindowClose {
            self.handle.mark_closed();
            let parts = self.session.parts_mut();
            parts.text_input.window_focused = false;
            sync_window_text_input(
                parts.tree,
                parts.active_work,
                parts.text_input,
                window_id,
                native_window,
                platform,
            );
            return false;
        }

        let parts = self.session.parts_mut();
        self.driver.handle_window_event(
            event,
            parts.tree,
            parts.engine,
            self._window.as_mut(),
            platform,
            parts.text_input,
        );

        if let Some(system_event) = map_ui_event(event) {
            parts.tree.dispatch_event(&system_event);
            if let Err(error) = apply_pending_window_actions(parts.tree, self._window.as_mut()) {
                crate::core::log::error_fn(format!(
                    "secondary window action failed: {}",
                    error.short_what()
                ));
                return false;
            }
        }
        platform.event_bus().publish(event);
        sync_window_text_input(
            parts.tree,
            parts.active_work,
            parts.text_input,
            window_id,
            native_window,
            platform,
        );
        true
    }

    fn drain_main_thread_work(&mut self) -> bool {
        let parts = self.session.parts_mut();
        let mut main_thread_context =
            MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        let had_main_thread_work = parts.main_thread_queue.drain(&mut main_thread_context);
        let had_app_state_semantic_work = parts.tree.drain_app_state_semantic_events();

        had_main_thread_work || had_app_state_semantic_work || *parts.reconcile_pending
    }

    fn has_frame_work(&mut self, now: Instant) -> bool {
        let parts = self.session.parts_mut();
        self.driver.has_frame_work(
            now,
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            &parts.main_thread_queue,
            parts.agent_commands,
            parts.pending_root,
            parts.reconcile_pending,
        )
    }

    fn drain_frame(
        &mut self,
        font_service: &FontService,
        image_service: &ImageService,
        theme: &RefCell<Theme>,
        debug_mode: &Cell<bool>,
        cursor_pos: &Cell<Point>,
        clock: &dyn AppClock,
        platform: Option<&mut dyn Platform>,
    ) -> bool {
        let now = clock.now();
        let Self {
            session,
            _window,
            driver,
            last_frame,
            ..
        } = self;
        let parts = session.parts_mut();
        let mut no_runtime_tasks = |_platform: &mut dyn Platform, _tree: &mut WidgetTree| {};
        let no_frame = |_tree: &mut WidgetTree,
                        _engine: &mut dyn GraphicsEngine,
                        _platform: &mut dyn Platform| {};
        let result = driver.drive_frame(WindowFrameContext {
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
            platform_window: _window.as_mut(),
            platform,
            font_service,
            image_service,
            theme,
            debug_mode,
            cursor_pos,
            metrics: None,
            now,
            had_events: false,
            had_layout_event: false,
            input_us: 0,
            next_external_deadline: None,
            on_runtime_tasks: &mut no_runtime_tasks,
            on_frame: &no_frame,
        });
        if let Err(error) = apply_pending_window_actions(parts.tree, _window.as_mut()) {
            crate::core::log::error_fn(format!(
                "secondary window action failed after runtime work: {}",
                error.short_what()
            ));
            report_window_operation_error(
                "secondary window action failure close request failed",
                _window.request_close(),
            );
        }
        *last_frame = driver.last_frame();
        result.did_work
    }

    fn next_deadline(&mut self) -> Option<Instant> {
        let parts = self.session.parts_mut();
        self.driver.next_deadline(
            Instant::now(),
            parts.tree,
            parts.active_work,
            &parts.app_timers,
            parts.agent_commands,
            parts.pending_root,
            *parts.reconcile_pending,
        )
    }

    fn close(mut self) {
        report_window_operation_error(
            "secondary graphics shutdown failed",
            self.session.try_shutdown(),
        );
        // Drop retries a failed checked shutdown while the native surface is
        // still alive. Successful shutdown is idempotent.
        drop(self.session);
        report_window_operation_error("secondary close failed", self._window.close());
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
    custom_title_bar: bool,
    theme: Theme,
    pub(crate) follow_system_theme: bool,
    app_state: AppState,
    pub(crate) app_timers: AppTimerQueue,
    pub(crate) main_thread_queue: MainThreadQueue,
    runtime: AppRuntime,
    handle_alive: Arc<AtomicBool>,
    root_factory: Option<Arc<dyn Fn() -> ViewNode + Send + Sync>>,
    pub(crate) on_start: Option<Box<dyn FnOnce(AppHandle) + Send>>,
    pub(crate) on_window_start: Option<Arc<dyn Fn(AppHandle) + Send + Sync>>,
    on_exit: Option<ExitPredicate>,
    cli: Option<Cli>,
    container: Container,
    settings_path: Option<String>,
    pub(crate) graphics_backend: Option<GraphicsBackend>,
    #[cfg(feature = "agent-control")]
    agent_control_enabled: bool,
    exit_code: i32,
}

type ExitPredicate = Box<dyn Fn(&UiEvent) -> bool>;

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
            custom_title_bar: false,
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
            graphics_backend: None,
            #[cfg(feature = "agent-control")]
            agent_control_enabled: false,
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

    /// 隐藏主窗口的系统标题栏，由根 View 自定义标题栏。
    pub fn custom_title_bar(mut self, enabled: bool) -> Self {
        self.custom_title_bar = enabled;
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

    /// Select the GPU API once during window/engine initialization.
    pub fn graphics_backend(mut self, backend: GraphicsBackend) -> Self {
        self.graphics_backend = Some(backend);
        self
    }

    /// Explicitly enables the process-wide Agent Bridge core for this GUI
    /// application. The build must also opt in to the `agent-control` feature.
    #[cfg(feature = "agent-control")]
    pub fn enable_agent_control(mut self) -> Self {
        self.agent_control_enabled = true;
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

    pub(crate) fn configured_graphics_backend(&self) -> GraphicsBackend {
        let env_value = std::env::var(GRAPHICS_BACKEND_ENV).ok();
        resolve_graphics_backend(
            self.graphics_backend,
            env_value.as_deref(),
            self.container.resolve::<SettingsService>(),
        )
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
    pub fn singleton<T: 'static + Send + Sync>(mut self, instance: T) -> Self {
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

    pub(crate) fn load_configured_settings(&mut self) -> bool {
        let Some(path) = self.settings_path.as_deref() else {
            return true;
        };

        let settings = SettingsService::new();
        let load_result = resolve_configured_settings_path(path)
            .and_then(|resolved_path| settings.load(&resolved_path));
        if let Err(err) = load_result {
            crate::core::log::error_fn(format!("load settings failed: {}", err.short_what()));
            self.exit_code = 1;
            return false;
        }

        self.container.singleton(settings);
        true
    }

    pub(crate) fn run_cli(&mut self) -> i32 {
        if let Some(ref mut cli) = self.cli {
            let args = std::env::args().collect::<Vec<_>>();
            self.exit_code = cli.run(&args);
        } else {
            crate::core::log::error_fn("CLI 模式须调用 .cli() 注册命令处理器");
            self.exit_code = 1;
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
        let graphics_backend = self.configured_graphics_backend();

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
        if self.custom_title_bar {
            if let Err(error) = configure_custom_title_bar(platform_window.as_mut(), w, h) {
                crate::core::log::error_fn(format!(
                    "configure custom title bar failed: {}",
                    error.short_what()
                ));
                report_window_operation_error(
                    "custom title bar failure cleanup close failed",
                    platform_window.close(),
                );
                return 1;
            }
        }
        report_window_operation_error(
            "initial center_on_screen failed",
            platform_window.center_on_screen(),
        );
        let engine = match create_preferred_engine(platform_window.as_mut(), w, h, graphics_backend)
        {
            Some(engine) => engine,
            None => {
                report_window_operation_error(
                    "initial engine failure cleanup close failed",
                    platform_window.close(),
                );
                return 1;
            }
        };
        #[cfg(feature = "agent-control")]
        if self.agent_control_enabled {
            let _ = self.runtime.enable_agent_control();
        }
        // 推迟 ShowWindow 到首帧 present 成功：否则 Vulkan/字体/首 layout 期间用户看到白屏。
        let event_loop_waker = platform.event_loop().waker();
        self.runtime.set_event_loop_waker(event_loop_waker.clone());
        self.app_state.set_event_loop_waker(event_loop_waker);

        let mut font_service = FontService::new();
        let font_t0 = std::time::Instant::now();
        font_service.load_default_system_font(14.0, platform.system_info());
        // Icon 依赖 Lucide PUA 字形；未加载时会回退为首字母。
        crate::ui::widgets::icon::init_lucide_font(
            include_bytes!("../../../assets/fonts/lucide.ttf"),
            &mut font_service,
        );
        crate::core::log::info_fn(format!(
            "startup fonts ready in {}ms (primary+CJK only; show deferred)",
            font_t0.elapsed().as_millis()
        ));
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

        let notifications = AppNotificationState::new();
        self.container.singleton(notifications.clone());

        let root_window_id = platform_window.window_id();
        let root_notifications = notifications.clone();
        let mut session = WindowSession::from_root_factory_for_window(
            root_window_id,
            move || {
                wrap_root_with_notification_overlay(
                    root_factory(),
                    root_notifications.clone(),
                    root_window_id,
                )
            },
            engine,
            w,
            h,
        );
        session.set_text_input_coordinator(self.runtime.text_input_coordinator());
        session.set_app_state(self.app_state.clone());
        session.set_app_timers(self.app_timers.clone());
        session.set_main_thread_queue(self.main_thread_queue.clone());
        self.runtime.register_session(
            root_window_id,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
        );
        if let Some(queue) = self.runtime.agent_command_queue(root_window_id) {
            session.set_agent_command_queue(queue);
        }
        if let Some(registration) = self.runtime.register_agent_window(
            root_window_id,
            self.title.clone(),
            platform_window.is_visible(),
            initially_agent_presentable(platform_window.as_ref()),
        ) {
            let _ = session.bind_agent_window(registration);
        }
        #[cfg(feature = "agent-control")]
        if self.agent_control_enabled {
            if let Err(error) = self.runtime.start_agent_transport() {
                crate::core::log::error_fn(format!("agent transport startup failed: {error}"));
                report_window_operation_error(
                    "agent transport failure graphics shutdown failed",
                    session.try_shutdown(),
                );
                drop(session);
                report_window_operation_error(
                    "agent transport failure window close failed",
                    platform_window.close(),
                );
                self.runtime.shutdown_all();
                return 1;
            }
        }
        let app_handle = self.app_handle_for_window(root_window_id);
        if let Some(on_start) = self.on_start.take() {
            on_start(app_handle.clone());
        }
        let secondary_windows = RefCell::new(Vec::new());
        let secondary_clock = system_clock();
        drain_pending_open_windows_with_backend(
            &mut *platform,
            &self.runtime,
            &self.app_state,
            &self.container,
            graphics_backend,
            self.on_window_start.as_ref(),
            &mut secondary_windows.borrow_mut(),
        );
        drain_secondary_window_queues(&mut secondary_windows.borrow_mut());

        let theme = RefCell::new(self.runtime.take_pending_theme().unwrap_or(self.theme));
        // UIX_DEBUG=1 启动即开调试 overlay（与 Window::new 一致）。
        let debug_mode = Cell::new(std::env::var("UIX_DEBUG").is_ok());
        let cursor_pos = Cell::new(Point::new(0.0, 0.0));
        let metrics = Cell::new(crate::draw::pipeline::RenderMetrics::default());
        drain_secondary_window_frames_with_platform(
            &mut *platform,
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
            Some(&metrics),
            map_ui_event,
            |ev| on_exit(ev),
            |platform, tree| {
                if let Some(next_theme) = runtime.take_pending_theme() {
                    apply_runtime_theme_change(
                        &theme,
                        tree,
                        &mut secondary_windows.borrow_mut(),
                        next_theme,
                    );
                }
                drain_pending_open_windows_with_backend(
                    platform,
                    &runtime,
                    &app_state,
                    &container,
                    graphics_backend,
                    on_window_start.as_ref(),
                    &mut secondary_windows.borrow_mut(),
                );
                drain_secondary_window_queues(&mut secondary_windows.borrow_mut());
                drain_secondary_window_frames_with_platform(
                    platform,
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
                if let UiEventType::ThemeChanged = event.type_ {
                    if let UiEventPayload::ThemeChanged(ref data) = event.payload {
                        dispatch_secondary_system_theme_changed(
                            &mut secondary_windows.borrow_mut(),
                            data.is_dark,
                        );
                    }
                } else {
                    dispatch_secondary_window_event(
                        &mut secondary_windows.borrow_mut(),
                        platform,
                        event,
                    );
                }
            },
            || secondary_windows_next_deadline(&mut secondary_windows.borrow_mut()),
            |_, _, _| {},
        );

        report_window_operation_error("main graphics shutdown failed", session.try_shutdown());
        // Keep the native window alive through the checked Drop retry.
        drop(session);
        report_window_operation_error("main close failed", platform_window.close());

        let mut secondary_windows = secondary_windows.into_inner();
        for window in secondary_windows.drain(..) {
            window.close();
        }
        self.runtime.shutdown_all();
        app_handle.mark_closed();

        0
    }
}

fn resolve_configured_settings_path(path: &str) -> crate::core::Result<String> {
    let configured = Path::new(path);
    let resolved = if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        let executable = std::env::current_exe()?;
        let executable_dir = executable.parent().ok_or_else(|| {
            Error::new(
                Errc::InvalidState,
                "settings: current executable has no parent directory",
            )
        })?;
        executable_dir.join(configured)
    };

    resolved.into_os_string().into_string().map_err(|_| {
        Error::new(
            Errc::FormatError,
            "settings: configured path is not valid UTF-8",
        )
    })
}

mod runtime;

pub use runtime::map_ui_event;
pub(crate) use runtime::{
    apply_runtime_theme_change, dispatch_secondary_system_theme_changed,
    dispatch_secondary_window_event, drain_pending_open_windows_with_backend,
    drain_secondary_window_queues, resolve_graphics_backend, secondary_windows_next_deadline,
};
use runtime::{create_preferred_engine, drain_secondary_window_frames_with_platform};
#[cfg(test)]
pub(crate) use runtime::{
    drain_pending_open_windows, drain_secondary_window_frames, format_gpu_probe_fallback,
    graphics_recovery_rebuilder,
};
