//! 应用入口 — 统一 GUI / CLI 生命周期。

use std::cell::{Cell, RefCell};
use std::path::Path;
use std::rc::Rc;
use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};

use crate::app::app_events::ThemeApplied;
use crate::app::application::app_handle::{
    wrap_root_with_notification_overlay, AppHandle, AppNotificationState,
};
use crate::app::application::cli::Cli;
use crate::app::application::di::Container;
use crate::app::event_loop::run_window_session_loop_with_system_theme_and_tasks;
use crate::app::queues::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::queues::clock::{system_clock, AppClock};
use crate::app::queues::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::session_runtime::{AppRuntime, OpenWindowRequest};
use crate::app::window::text_input::sync_window_text_input;
use crate::app::window::window_actions::{
    apply_pending_window_actions, configure_custom_title_bar,
};
use crate::app::window::window_config::WindowConfig;
use crate::app::window::window_driver::{WindowDriver, WindowFrameContext};
use crate::app::window::window_session::WindowSession;
use crate::bus::EventBus;
use crate::core::{Errc, Error, Point, WindowId};
use crate::data::SettingsService;
use crate::draw::renderer::bootstrap::{
    assemble_renderer, bootstrap_renderer_with_pending, ProbeReport,
};
#[cfg(feature = "test-harness")]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::draw::renderer::{RebuildRequest, RecoveryDriver, RenderTargetRebuilder};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::target::RenderTarget;
use crate::draw::Renderer;
use crate::native::factory::{
    create_platform_with_pending, gpu_recipe_candidates, graphics_runtime_platform,
    try_create_gpu_recipe_with_queue, GraphicsRecipe,
};
use crate::native::platform::Platform;
use crate::native::present::{GraphicsApi, GraphicsSelection, NativeSurfaceHandle};
use crate::native::windowing::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::windowing::window::{PlatformWindow, WindowOcclusionState};
use crate::platform::graphics::GraphicsBackend;
use crate::ui::theme::traits::TokenProvider;
use crate::ui::theme::{DesignTokens, DynTokens, Theme};
use crate::ui::view::ViewNode;
use crate::ui::{
    with_config, with_locale, AppState, ComponentConfig, Locale, SystemEvent, WidgetTree,
};

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
        tracing::warn!("{context}: {}", error.short_what());
    }
}

fn initially_agent_presentable(window: &dyn PlatformWindow) -> bool {
    let properties = window.properties();
    properties.width() > 0
        && properties.height() > 0
        && !properties.is_minimized()
        && window.occlusion_state() != WindowOcclusionState::Occluded
}

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
    /// 保存公开 builder 已显式选择的内部具体 API 身份。
    pub(crate) graphics_backend: Option<GraphicsApi>,
    #[cfg(feature = "test-harness")]
    graphics_faults: GraphicsFaultSignal,
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
            #[cfg(feature = "test-harness")]
            graphics_faults: GraphicsFaultSignal::default(),
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

    /// 设置所有窗口根 View 的默认语言；子树可由 `LocaleProvider` 覆写。
    pub fn locale(mut self, locale: Locale) -> Self {
        self.container.singleton(locale);
        self
    }

    /// 设置所有窗口根 View 的组件默认配置；子树可由 `ConfigProvider` 覆写。
    pub fn config(mut self, config: ComponentConfig) -> Self {
        self.container.singleton(config);
        self
    }

    /// Configures the runtime-scoped stability, recovery, and reporting
    /// service before the application starts.
    pub fn diagnostics(mut self, config: crate::diagnostics::DiagnosticsConfig) -> Self {
        self.runtime.set_diagnostics(config);
        self
    }

    /// 注入已构建的 Diagnostics runtime 实例（宿主持有共享句柄，如
    /// `Diagnostics::new(DiagnosticsConfig::default())`）。与 [`Self::diagnostics`]
    /// 互斥；后调用者生效。
    pub fn diagnostics_runtime(mut self, diagnostics: crate::diagnostics::Diagnostics) -> Self {
        self.runtime.set_diagnostics_runtime(diagnostics);
        self
    }

    /// 设置是否在运行中跟随 OS 主题变化（默认 false）。
    pub fn follow_system_theme(mut self, follow: bool) -> Self {
        self.follow_system_theme = follow;
        self
    }

    /// Select the GPU API once during window/engine initialization.
    pub fn graphics_backend(mut self, backend: GraphicsBackend) -> Self {
        self.graphics_backend = Some(backend.into_native());
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

    /// 按 builder、环境变量与设置的优先级解析私有启动策略。
    pub(crate) fn configured_graphics_backend(&self) -> GraphicsSelection {
        let env_value = std::env::var(GRAPHICS_BACKEND_ENV).ok();
        resolve_graphics_backend(
            self.graphics_backend,
            env_value.as_deref(),
            self.container.resolve::<SettingsService>(),
        )
    }

    // 测试目标保留根窗口句柄快捷入口，供外部 GUI 测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
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

    /// 批量注册服务对象（E-08）：把应用自定义聚合服务（如 `AppServices`）
    /// 整体注册，组件经 `resolve` 取用，不各自 new 全局服务。
    pub fn register<T: 'static + Send + Sync>(mut self, instance: T) -> Self {
        self.container.register(instance);
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
        // panic hook 在配置加载前安装：settings / CLI / GUI 任一段 panic 都被
        // 捕获并原子写入 crash report（配置目录时），且从不吞 panic。
        self.runtime.diagnostics().install_panic_hook();
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
            tracing::error!("load settings failed: {}", err.short_what());
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
            tracing::error!("CLI 模式须调用 .cli() 注册命令处理器");
            self.exit_code = 1;
        }
        self.exit_code
    }

    fn run_gui(mut self) -> i32 {
        let root_factory = match self.root_factory.take() {
            Some(factory) => factory,
            None => {
                tracing::error!("GUI 模式须调用 .root() 设置根 View");
                return 1;
            }
        };

        let (w, h) = self.size;
        let graphics_backend = self.configured_graphics_backend();
        let diagnostics = self.runtime.diagnostics();

        // 真实 device-lost 恢复注册：typed `GraphicsDeviceLost` 到达 owner-thread
        // 安全点时，恢复 handler 只请求窗口引擎在下个帧边界执行既有有界恢复序列
        // （领域算法仍归 graphics 的 RecoveryDriver）；`report` 不自动执行恢复。
        let recovery_request = RebuildRequest::default();
        let _device_lost_recovery = diagnostics.on_error(Errc::GraphicsDeviceLost, {
            let request = recovery_request.clone();
            move |_error| {
                request.request_rebuild();
                crate::diagnostics::RecoveryAction::Recovered
            }
        });

        let mut platform = match create_platform_with_pending(diagnostics.pending_failure_queue()) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("create_platform 失败: {:?}", e);
                return 1;
            }
        };
        drain_platform_pending_failures(&mut *platform, &diagnostics);

        let mut platform_window = match platform.window_manager().create_window(&self.title, w, h) {
            Ok(win) => win,
            Err(e) => {
                tracing::error!("create_window 失败: {:?}", e);
                return 1;
            }
        };
        if self.custom_title_bar {
            if let Err(error) = configure_custom_title_bar(platform_window.as_mut(), w, h) {
                tracing::error!("configure custom title bar failed: {}", error.short_what());
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
        drain_platform_pending_failures(&mut *platform, &diagnostics);
        #[cfg(feature = "test-harness")]
        let preferred_engine = create_preferred_engine(
            platform_window.as_mut(),
            w,
            h,
            graphics_backend,
            self.runtime.diagnostics(),
            recovery_request.clone(),
            self.graphics_faults.clone(),
        );
        #[cfg(not(feature = "test-harness"))]
        let preferred_engine = create_preferred_engine(
            platform_window.as_mut(),
            w,
            h,
            graphics_backend,
            self.runtime.diagnostics(),
            recovery_request.clone(),
        );
        let engine = match preferred_engine {
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
            include_bytes!("../../../../assets/fonts/lucide.ttf"),
            &mut font_service,
        );
        tracing::info!(
            "startup fonts ready in {}ms (primary+CJK only; show deferred)",
            font_t0.elapsed().as_millis()
        );
        let image_service = ImageService::new();

        let system_theme_tokens = if self.follow_system_theme {
            let is_dark = platform.display().is_dark_mode().unwrap_or_else(|error| {
                tracing::warn!(
                    "startup theme query failed, defaulting to light: {}",
                    error.short_what()
                );
                false
            });
            let tokens = Arc::new(DynTokens::new(if is_dark {
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
        let locale = self.container.resolve_clone::<Locale>().unwrap_or_default();
        let component_config = self
            .container
            .resolve_clone::<ComponentConfig>()
            .unwrap_or_default();

        let root_window_id = platform_window.window_id();
        let root_notifications = notifications.clone();
        let mut session = WindowSession::from_root_factory_for_window(
            root_window_id,
            move || {
                with_config(&component_config, || {
                    with_locale(&locale, || {
                        wrap_root_with_notification_overlay(
                            root_factory(),
                            root_notifications.clone(),
                            root_window_id,
                        )
                    })
                })
            },
            engine,
            w,
            h,
        );
        session.set_text_input_coordinator(self.runtime.text_input_coordinator());
        session.set_app_state(self.app_state.clone());
        session.set_app_timers(self.app_timers.clone());
        session.set_main_thread_queue(self.main_thread_queue.clone());
        #[cfg(feature = "test-harness")]
        self.runtime.register_session_with_graphics_faults(
            root_window_id,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
            self.graphics_faults.clone(),
        );
        #[cfg(not(feature = "test-harness"))]
        self.runtime.register_session(
            root_window_id,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
        );
        if let Some(queue) = self.runtime.agent_command_queue(root_window_id) {
            session.set_agent_command_queue(queue);
        }
        session.set_agent_command_executor(self.runtime.agent_command_executor());
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
                tracing::error!("agent transport startup failed: {error}");
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

        // 主题事实总线（System 私有边界，app_events）：组合根/应用组装处
        // 创建并注入窄能力；订阅在组装期注册，路由见 docs/架构/路由清单.md。
        let theme_bus = Rc::new(RefCell::new(EventBus::new()));
        // 主题应用序次（单调递增，供 ThemeApplied 负载区分重复应用）。
        let theme_revision = Cell::new(0u64);
        // 感知方：主题变更诊断遥测（tracing 发射，可观测性订阅）。
        let theme_events_subscription = match theme_bus
            .borrow_mut()
            .subscribe(|fact: &ThemeApplied| {
                tracing::debug!(
                    target: "app.theme",
                    is_dark = fact.is_dark,
                    revision = fact.revision,
                    "theme applied"
                );
            }) {
            Ok(subscription) => subscription,
            Err(error) => {
                // 新总线必为 Active，此处不可达；若发生（实现缺陷）
                // 中止启动并报告，不静默吞掉订阅失败。
                tracing::error!("theme event subscription failed: {}", error.short_what());
                return 1;
            }
        };
        drain_pending_open_windows_with_backend(
            &mut *platform,
            &self.runtime,
            &self.app_state,
            &self.container,
            graphics_backend,
            recovery_request.clone(),
            self.on_window_start.as_ref(),
            &mut secondary_windows.borrow_mut(),
        );
        drain_secondary_window_queues(&mut secondary_windows.borrow_mut());

        let theme = RefCell::new(self.runtime.take_pending_theme().unwrap_or(self.theme));
        // UIX_DEBUG=1 启动即开调试 overlay（与 Window::new 一致）。
        let debug_mode = Cell::new(std::env::var("UIX_DEBUG").is_ok());
        let cursor_pos = Cell::new(Point::new(0.0, 0.0));
        let metrics = Cell::new(crate::draw::renderer::RenderMetrics::default());
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
                drain_platform_pending_failures(platform, &diagnostics);
                if let Some(next_theme) = runtime.take_pending_theme() {
                    apply_runtime_theme_change(
                        &theme,
                        tree,
                        &mut secondary_windows.borrow_mut(),
                        next_theme,
                    );
                    // 事实建立点：主题替换完成后发布 ThemeApplied
                    // （SystemEvent(SMC)）；失败或回滚时不得发布。
                    theme_revision.set(theme_revision.get() + 1);
                    if let Err(error) = theme_bus.borrow().publish(ThemeApplied {
                        is_dark: theme.borrow().is_dark(),
                        revision: theme_revision.get(),
                    }) {
                        tracing::error!("theme applied publish failed: {}", error.short_what());
                    }
                }
                drain_pending_open_windows_with_backend(
                    platform,
                    &runtime,
                    &app_state,
                    &container,
                    graphics_backend,
                    recovery_request.clone(),
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
                        // 事实建立点：系统主题已生效并分发给全部窗口 UI 树后
                        // 发布 ThemeApplied（SystemEvent(SMC)）；本闭包只由主窗口
                        // 循环转发调用，window_id=None 事件不进 foreign_events
                        // 队列，同一事实仅发布一次。
                        theme_revision.set(theme_revision.get() + 1);
                        if let Err(error) = theme_bus.borrow().publish(ThemeApplied {
                            is_dark: data.is_dark,
                            revision: theme_revision.get(),
                        }) {
                            tracing::error!(
                                "theme applied publish failed: {}",
                                error.short_what()
                            );
                        }
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

        // 会话循环结束：释放主题事实订阅句柄（幂等注销）。
        drop(theme_events_subscription);

        report_window_operation_error("main graphics shutdown failed", session.try_shutdown());
        // Keep the native window alive through the checked Drop retry.
        drop(session);
        report_window_operation_error("main close failed", platform_window.close());

        let mut secondary_windows = secondary_windows.into_inner();
        for window in secondary_windows.drain(..) {
            window.close();
        }
        drain_platform_pending_failures(&mut *platform, &diagnostics);
        self.runtime.shutdown_all();
        app_handle.mark_closed();

        0
    }
}

fn resolve_configured_settings_path(path: &str) -> crate::core::Result<String> {
    if path.trim().is_empty() {
        return Err(Error::invalid_arg("settings: path must not be empty"));
    }
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

mod secondary;

use self::secondary::SecondaryWindowSession;

mod runtime;

pub use runtime::map_ui_event;
pub(crate) use runtime::{
    apply_runtime_theme_change, dispatch_secondary_system_theme_changed,
    dispatch_secondary_window_event, drain_pending_open_windows_with_backend,
    drain_secondary_window_queues, resolve_graphics_backend, secondary_windows_next_deadline,
};
use runtime::{
    create_preferred_engine, drain_platform_pending_failures,
    drain_secondary_window_frames_with_platform,
};
