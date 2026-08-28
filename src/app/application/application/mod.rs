//! 应用入口 — 统一 GUI / CLI 生命周期。

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::{Duration, Instant};

use crate::app::agent::agent_policy::AgentPolicy;
use crate::app::app_events::ThemeApplied;
use crate::app::application::app_handle::{AppHandle, prepare_app_root};
use crate::app::application::cli::Cli;
use crate::app::application::di::Container;
// 引入 Application System 私有逐窗反馈 owner。
use crate::app::application::feedback_state::AppFeedbackState;
// 引入 App 作用域的 UIX 具名主题表。
use crate::app::application::named_themes::NamedThemes;
use crate::app::event_loop::run_window_session_loop_with_system_theme_and_tasks;
use crate::app::queues::agent_command_queue::AgentConfirmationRequest;
use crate::app::queues::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::queues::clock::{AppClock, system_clock};
// Application 组合根只保留创建窗口队列所需的 MainThreadQueue 类型。
use crate::app::queues::main_thread_queue::MainThreadQueue;
use crate::app::session_runtime::{AppRuntime, OpenWindowRequest};
use crate::app::window::text_input::sync_window_text_input;
use crate::app::window::window_actions::{
    apply_pending_window_actions,
    configure_custom_title_bar,
    // 统一主窗与次窗的自动居中能力缺失策略。
    report_center_on_screen_result,
};
// 统一主窗、次窗与公开 Window 的 FileDrop 创建策略。
use crate::app::window::window_config::WindowConfig;
use crate::app::window::window_creation::create_app_window;
use crate::app::window::window_driver::{WindowDriver, WindowFrameContext};
use crate::app::window::window_session::WindowSession;
use crate::bus::EventBus;
use crate::core::{Errc, Error, Point, WindowId};
use crate::data::SettingsService;
use crate::draw::Renderer;
use crate::draw::renderer::bootstrap::{
    ProbeReport, assemble_renderer, bootstrap_renderer_with_pending,
};
#[cfg(any(feature = "test-harness", feature = "agent-control"))]
use crate::draw::renderer::test_harness::GraphicsFaultSignal;
use crate::draw::renderer::{RebuildRequest, RecoveryDriver, RenderTargetRebuilder};
// 子模块通过父边界复用所有窗口共享的字体服务类型。
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::draw::target::RenderTarget;
// Application 只把运行时故障队列交给平台中立入口，不选择具体原生后端。
use crate::platform::graphics::GraphicsBackend;
use crate::platform::platform::PlatformSystem;
use crate::platform::presentation::{
    GraphicsApi, GraphicsRecipe, GraphicsSelection, NativeSurfaceHandle, gpu_recipe_candidates,
    graphics_runtime_platform, try_create_gpu_recipe_with_queue,
};
use crate::platform::windowing::event::{UiEvent, UiEventPayload, UiEventType};
use crate::platform::windowing::window::{PlatformWindow, WindowOcclusionState};
use crate::platform::{PendingNativeOptions, create_platform_with_pending};
use crate::ui::semantic_action::SemanticActionKind;
use crate::ui::theme::traits::TokenProvider;
use crate::ui::theme::{DesignTokens, DynTokens, Theme};
use crate::ui::view::ViewNode;
use crate::ui::{
    AppState, Locale, SystemEvent, WidgetConfig, WidgetTree, with_config, with_locale,
};

// ════════════════════════════════════════════════════════════════════════════
// 应用模式
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
/// 应用的运行入口模式。
pub enum AppMode {
    #[default]
    /// 创建窗口并运行图形界面事件循环。
    GUI,
    /// 运行命令行入口而不创建图形界面窗口。
    CLI,
}

const GRAPHICS_BACKEND_ENV: &str = "UIX_GRAPHICS_BACKEND";
const GRAPHICS_BACKEND_SETTING_KEYS: [&str; 2] = ["graphics_backend", "uix.graphics_backend"];

// 默认构造和窗口启动 helper 保持在 application Module 内部，避免组合根文件超限。
mod defaults;
// 组合根与运行时子模块复用同一检查式窗口错误边界。
use self::defaults::{initially_agent_presentable, report_window_operation_error};
// 字体配置注入和资源服务组装保持在独立组合根边界。
mod typography;
// GUI 启动只消费已验证的完整字体服务。
use self::typography::initialize_font_service;

/// 应用入口：GUI（View 根节点）或 CLI 模式。
pub struct App {
    mode: AppMode,
    title: String,
    size: (i32, i32),
    custom_title_bar: bool,
    theme: Theme,
    // 保存当前 App 的 UIX 具名主题，不与其他 App 共享可变注册表。
    named_themes: NamedThemes,
    pub(crate) follow_system_theme: bool,
    app_state: AppState,
    pub(crate) app_timers: AppTimerQueue,
    pub(crate) main_thread_queue: MainThreadQueue,
    runtime: AppRuntime,
    /// 公开 builder 对运行时调试模式的显式覆写；空值保留 Diagnostics 配置或环境值。
    debug_mode_override: Option<bool>,
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
    #[cfg(any(feature = "test-harness", feature = "agent-control"))]
    graphics_faults: GraphicsFaultSignal,
    #[cfg(feature = "agent-control")]
    agent_control_enabled: bool,
    /// Agent 动作策略（授权第二层）：默认全放行，应用按需收紧。
    agent_policy: AgentPolicy,
    /// Agent 确认 UI 回调（授权第三层）：默认不注入（确认请求直接失败）。
    agent_confirm_ui: Option<Arc<dyn Fn(AgentConfirmationRequest) + Send + Sync>>,
    exit_code: i32,
}

type ExitPredicate = Box<dyn Fn(&UiEvent) -> bool>;

impl App {
    /// 创建使用默认图形界面模式和运行时配置的应用。
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

    /// 安装 UIX 编译器生成的 App 作用域主题表与初始主题。
    #[doc(hidden)]
    pub fn __uix_named_themes<I>(mut self, themes: I, initial: &str) -> Self
    where
        // 接受宏生成的静态名称与拥有所有权主题序列。
        I: IntoIterator<Item = (&'static str, Theme)>,
    {
        // 文档主题按声明结果覆盖同名内建预设。
        for (name, theme) in themes {
            // 写入 App 私有主题表。
            self.named_themes.insert(name.to_string(), theme);
        }
        // 宏已在编译期验证名称；运行期仍避免不可恢复 panic。
        if let Some(theme) = self.named_themes.resolve(initial) {
            // 安装初始主题。
            self.theme = theme;
        }
        // 返回可继续链式配置的同一 App builder。
        self
    }

    /// 设置所有窗口根 View 的默认语言；子树可由 `LocaleProvider` 覆写。
    pub fn locale(mut self, locale: Locale) -> Self {
        self.container.singleton(locale);
        self
    }

    /// 设置所有窗口根 View 的组件默认配置；子树可由 `ConfigProvider` 覆写。
    pub fn config(mut self, config: WidgetConfig) -> Self {
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

    /// 显式设置应用启动时的统一调试模式。
    ///
    /// 该设置优先于 `UIX_DEBUG`，并在全部窗口间共享。运行中可通过
    /// `Ctrl+Shift+D` 或 [`crate::diagnostics::Diagnostics::set_debug_mode`] 切换。
    pub fn debug_mode(mut self, enabled: bool) -> Self {
        self.debug_mode_override = Some(enabled);
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

    /// 只读授权：Agent 只能读取窗口与语义快照，不能执行任何动作。
    ///
    /// 默认策略是「全放行」——已通过连接鉴权的 Agent 可以自由控制应用的
    /// 全部能力；本方法及以下策略方法用于按需收紧。
    pub fn agent_read_only(mut self) -> Self {
        self.agent_policy = self.agent_policy.read_only();
        self
    }

    /// 保护指定 `automation_id` 的组件：Agent 不能对其执行语义写动作。
    pub fn agent_protect(mut self, automation_id: impl Into<String>) -> Self {
        self.agent_policy = self.agent_policy.protect(automation_id);
        self
    }

    /// 全局禁止指定语义动作类别（如 `SemanticActionKind::SetValue`）。
    pub fn agent_deny_action(mut self, kind: SemanticActionKind) -> Self {
        self.agent_policy = self.agent_policy.deny_action(kind);
        self
    }

    /// 指定 `automation_id` 的组件需要用户确认：AI 对其执行写动作前进入
    /// 确认流程（返回 `requires_confirmation`，AI 随后发起 `confirm` 请求）。
    pub fn agent_require_confirm(mut self, automation_id: impl Into<String>) -> Self {
        self.agent_policy = self.agent_policy.require_confirm(automation_id);
        self
    }

    /// 注入 Agent 确认 UI 回调：AI 请求确认时在 UI turn 内调用，应用展示
    /// 自己的确认界面；用户决定通过 `AppHandle::resolve_agent_confirmation`
    /// 交回框架。未注入时确认请求直接失败（`confirmation_not_found`）。
    pub fn agent_confirm_ui(
        mut self,
        handler: impl Fn(AgentConfirmationRequest) + Send + Sync + 'static,
    ) -> Self {
        self.agent_confirm_ui = Some(Arc::new(handler));
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

    /// 设置主窗口启动后执行一次的应用级回调。
    pub fn on_start<F>(mut self, f: F) -> Self
    where
        F: FnOnce(AppHandle) + Send + 'static,
    {
        self.on_start = Some(Box::new(f));
        self
    }

    /// 设置每个窗口启动时执行的回调。
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

    /// 返回当前应用运行模式。
    pub fn current_mode(&self) -> AppMode {
        self.mode
    }

    /// 返回配置的窗口标题。
    pub fn window_title(&self) -> &str {
        &self.title
    }

    /// 返回配置的窗口初始宽高。
    pub fn window_size(&self) -> (i32, i32) {
        self.size
    }

    /// 返回当前应用退出码。
    pub fn exit_code(&self) -> i32 {
        self.exit_code
    }

    /// 返回应用拥有的依赖注入容器。
    pub fn container(&self) -> &Container {
        &self.container
    }

    /// 返回共享应用状态的句柄副本。
    pub fn app_state(&self) -> AppState {
        self.app_state.clone()
    }

    // ── 运行 ──────────────────────────────────────────────────────

    /// 加载已配置设置并按当前模式运行应用，返回最终退出码。
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
        let load_result = crate::data::settings::resolve_configured_settings_path(path)
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
        let requested_debug_mode = self
            .debug_mode_override
            .map(Ok)
            .or_else(crate::diagnostics::debug_mode_from_env);
        match requested_debug_mode {
            Some(Ok(enabled)) => diagnostics.set_debug_mode(enabled),
            Some(Err(())) => tracing::warn!(
                target: "uix::diagnostics",
                debug_event = "invalid_debug_switch",
                "UIX_DEBUG must be one of 1/0, true/false, yes/no, or on/off"
            ),
            None => {}
        }

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

        let pending_native = PendingNativeOptions::new(diagnostics.pending_failure_queue());
        let mut platform = match create_platform_with_pending(pending_native) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("create_platform 失败: {:?}", e);
                return 1;
            }
        };
        drain_platform_pending_failures(&mut *platform, &diagnostics);
        // 在创建窗口和图形设备前建立唯一字体服务，配置错误无需清理原生资源。
        let font_service =
            match initialize_font_service(&mut self.container, platform.system_info()) {
                // 成功后所有窗口共享同一字体句柄和 fallback 顺序。
                Ok(font_service) => font_service,
                // 确定性字体包失败必须终止启动，不能偷偷恢复平台字体。
                Err(error) => {
                    // 在最终责任边界记录不含字体数据的 typed 原因。
                    tracing::error!("startup font initialization failed: {}", error.what());
                    // 此时尚未创建窗口或图形资源，可直接返回失败退出码。
                    return 1;
                }
            };
        let mut platform_window =
            match create_app_window(platform.window_manager(), &self.title, w, h) {
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
        // Wayland 的预期不支持由 compositor 默认放置，不记录误导警告。
        report_center_on_screen_result(
            "initial center_on_screen failed",
            platform_window.center_on_screen(),
        );
        drain_platform_pending_failures(&mut *platform, &diagnostics);
        #[cfg(any(feature = "test-harness", feature = "agent-control"))]
        let preferred_engine = create_preferred_engine(
            platform_window.as_mut(),
            w,
            h,
            graphics_backend,
            self.runtime.diagnostics(),
            recovery_request.clone(),
            self.graphics_faults.clone(),
        );
        #[cfg(not(any(feature = "test-harness", feature = "agent-control")))]
        let preferred_engine = create_preferred_engine(
            platform_window.as_mut(),
            w,
            h,
            graphics_backend,
            self.runtime.diagnostics(),
            recovery_request.clone(),
        );
        let engine = match preferred_engine {
            Ok(engine) => engine,
            Err(error) => {
                // 在关闭原生窗口前记录完整的图形初始化与候选清理原因链。
                tracing::error!("initial graphics initialization failed: {}", error.what());
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
        // 组装期注入 Agent 动作策略（授权第二层），窗口创建前生效。
        self.runtime
            .set_agent_policy(std::mem::take(&mut self.agent_policy));
        // 组装期注入 Agent 确认 UI 回调（授权第三层），窗口创建前生效。
        if let Some(handler) = self.agent_confirm_ui.take() {
            self.runtime
                .set_agent_confirm_ui(move |request| handler(request));
        }
        // 推迟 ShowWindow 到首帧 present 成功：否则图形初始化、字体和首 layout 期间用户看到白屏。
        let event_loop_waker = platform.event_loop().waker();
        self.runtime.set_event_loop_waker(event_loop_waker.clone());
        self.app_state.set_event_loop_waker(event_loop_waker);

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

        // Application System 创建唯一反馈 owner，所有窗口只分配各自句柄组。
        let feedback = AppFeedbackState::new();
        // 通过 DI 共享同一个 owner，不向组件暴露全局注册表。
        self.container.singleton(feedback.clone());
        let locale = window_assembly::resolve_or_default::<Locale>(&self.container);
        let widget_config = window_assembly::resolve_or_default::<WidgetConfig>(&self.container);

        let root_window_id = platform_window.window_id();
        // 根窗口工厂捕获 owner，而不是捕获某个临时 Host 实例；包装顺序
        // （WidgetConfig → Locale → prepare_app_root）与副窗共用同一原语。
        let root_feedback = feedback.clone();
        let wrapped_root = window_assembly::wrap_app_root(
            &widget_config,
            &locale,
            root_window_id,
            Some(root_feedback),
            move || root_factory(),
        );
        let mut session = WindowSession::from_root_factory_for_window(
            root_window_id,
            wrapped_root,
            engine,
            w,
            h,
        );
        #[cfg(any(feature = "test-harness", feature = "agent-control"))]
        self.runtime.register_session_with_graphics_faults(
            root_window_id,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
            self.graphics_faults.clone(),
        );
        #[cfg(not(any(feature = "test-harness", feature = "agent-control")))]
        self.runtime.register_session(
            root_window_id,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            self.handle_alive.clone(),
        );
        // 会话资源接线与 Agent 注册经共享装配原语执行（副窗同序）。
        window_assembly::assemble_session_resources(
            &mut session,
            &self.runtime,
            &self.app_state,
            self.app_timers.clone(),
            self.main_thread_queue.clone(),
            root_window_id,
            &self.title,
            platform_window.as_ref(),
        );
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
        // 事实建立点统一收口：主题替换完成后发布 ThemeApplied（SystemEvent）；
        // 失败或回滚时不得调用。App 主题与系统主题两个来源共用同一发布管线。
        let publish_theme_applied = {
            let theme_bus = &theme_bus;
            let theme_revision = &theme_revision;
            move |is_dark: bool| {
                theme_revision.set(theme_revision.get() + 1);
                if let Err(error) = theme_bus.borrow().publish(ThemeApplied {
                    is_dark,
                    revision: theme_revision.get(),
                }) {
                    tracing::error!("theme applied publish failed: {}", error.short_what());
                }
            }
        };
        // 感知方：主题变更诊断遥测（tracing 发射，可观测性订阅）。
        let theme_events_subscription =
            match theme_bus.borrow_mut().subscribe(|fact: &ThemeApplied| {
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
        let theme = RefCell::new(self.runtime.take_pending_theme().unwrap_or(self.theme));
        // Diagnostics System 是全部窗口调试状态的唯一所有者。
        let debug_mode = diagnostics.clone();
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

        // 安装 uix-lang setTheme 内置操作使用的主题请求通道：
        // 事件处理器按名称提交主题，由既有 runtime.set_theme 通道在下一轮
        // runtime tasks 中应用并发布 ThemeApplied。
        let theme_requester_runtime = self.runtime.clone();
        // 克隆当前 App 的具名主题快照供事件循环请求器解析。
        let named_themes = self.named_themes.clone();
        // 注册到当前 UI 线程的窗口循环作用域。
        crate::ui::__private::uix_install_theme_requester(Box::new(move |name: &str| {
            // 未登记名称保持当前主题，并暴露可检索诊断。
            let Some(theme) = named_themes.resolve(name) else {
                // 记录错误名称，不再静默回退到亮色主题。
                tracing::warn!(theme_name = name, "uix-lang requested unknown App theme");
                // 结束本次无效请求。
                return;
            };
            // 通过公开 App 级主题通道提交切换。
            theme_requester_runtime.set_theme(theme);
        }));

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
                    publish_theme_applied(theme.borrow().is_dark());
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
                        // 发布 ThemeApplied；本闭包只由主窗口
                        // 循环转发调用，window_id=None 事件不进 foreign_events
                        // 队列，同一事实仅发布一次。
                        publish_theme_applied(data.is_dark);
                    }
                } else {
                    dispatch_secondary_window_event(
                        &mut secondary_windows.borrow_mut(),
                        platform,
                        event,
                    );
                }
            },
            || {
                let now = secondary_clock.now();
                secondary_windows_next_deadline(&mut secondary_windows.borrow_mut(), now)
            },
            |_, _, _| {},
        );

        // 主窗口循环一结束就发布精确 generation 的 closed 事实；后续图形与平台清理不得阻塞 wait。
        app_handle.mark_closed();

        // 会话循环结束：卸载主题请求通道并释放主题事实订阅句柄。
        crate::ui::__private::uix_clear_theme_requester();
        // 幂等注销主题事实订阅。
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

        0
    }
}

// 次要窗口会话模型由运行时节拍直接消费（经本模块 re-export 给子模块）。
mod secondary;

// 主窗/副窗共用的窗口装配原语（DI 回退、根工厂包装、会话接线）。
mod window_assembly;

use self::secondary::SecondaryWindowSession;

// 保留 `application::application::runtime` 逻辑路径，运行时物理归入 lifecycle。
#[path = "../lifecycle/runtime/mod.rs"]
mod runtime;

pub use runtime::map_ui_event;
pub(crate) use runtime::{
    apply_runtime_theme_change, dispatch_secondary_system_theme_changed,
    dispatch_secondary_window_event, drain_pending_open_windows_with_backend,
    secondary_windows_next_deadline,
};
use runtime::{
    create_preferred_engine, drain_platform_pending_failures,
    drain_secondary_window_frames_with_platform,
};
