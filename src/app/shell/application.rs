//! 应用入口 — 统一 GUI / CLI 生命周期。

use crate::ui::core::widget::WidgetCore;
use std::cell::{Cell, RefCell};
use std::sync::{atomic::AtomicBool, Arc};
use std::time::{Duration, Instant};

use crate::app::active_work_registry::{ActiveWorkKind, ActiveWorkRegistry};
use crate::app::app_handle::{
    wrap_root_with_notification_overlay, AppHandle, AppNotificationState,
};
use crate::app::app_timer::{AppTimerQueue, TimerHandle};
use crate::app::event_loop::run_window_session_loop_with_system_theme_and_tasks;
use crate::app::main_thread_queue::{MainThreadContext, MainThreadQueue};
use crate::app::session_runtime::{AppRuntime, OpenWindowRequest};
use crate::app::shell::cli::Cli;
use crate::app::shell::di::Container;
use crate::app::clock::{system_clock, AppClock};
use crate::app::window_config::WindowConfig;
use crate::app::window_session::{WindowLoopState, WindowSession};
use crate::core::{Errc, Error, Point, WindowId};
use crate::data::SettingsService;
use crate::draw::engine::bootstrap::{
    assemble_graphics_engine, bootstrap_graphics_engine, ProbeReport,
};
use crate::draw::engine::{GraphicsEngineRebuilder, RecoveringGraphicsEngine, RecoveryAction};
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::draw::painting::ThemeSnapshot;
use crate::draw::pipeline::{FrameRenderInput, FrameRenderer, InvalidationSource, NodeId};
use crate::draw::traits::GraphicsEngine;
use crate::draw::RenderOutcome;
use crate::draw::SoftwareEngine;
use crate::native::create_platform;
use crate::native::factory::{
    gpu_recipe_candidates, graphics_runtime_platform, try_create_gpu_recipe, GraphicsRecipe,
};
use crate::native::traits::event::{UiEvent, UiEventPayload, UiEventType};
use crate::native::traits::platform::Platform;
use crate::native::traits::present::{GraphicsBackend, NativeSurfaceHandle};
use crate::native::traits::window::PlatformWindow;
use crate::ui::theme::{DesignTokens, DynTokens, Theme};
use crate::ui::traits::TokenProvider;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::{AppState, EventResult, SystemEvent, WidgetTree};

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

fn report_graphics_resize_error(context: &str, result: crate::core::Result<()>) -> bool {
    match result {
        Ok(()) => true,
        Err(error) => {
            crate::core::log::warn_fn(format!("{context}: {}", error.short_what()));
            false
        }
    }
}

pub(crate) struct SecondaryWindowSession {
    // session 必须先于原生窗口析构，确保 engine/GL 资源先释放。
    pub(crate) session: WindowSession,
    _window: Box<dyn PlatformWindow>,
    pub(crate) handle: AppHandle,
    frame_renderer: FrameRenderer,
    rendered_first: bool,
    pub(crate) last_frame: Option<Instant>,
    window_visible: bool,
    initial_size: (i32, i32),
    /// 首帧 present 成功后再 ShowWindow，避免空内容白屏。
    deferred_show: bool,
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
                        let resized = report_graphics_resize_error(
                            "secondary graphics resize failed",
                            parts.engine.resize(d.width, d.height),
                        );
                        report_window_operation_error(
                            "secondary resize_notify failed",
                            self._window.resize_notify(d.width, d.height),
                        );
                        if resized {
                            let (cw, ch) = {
                                let canvas = parts.engine.canvas_2d();
                                (canvas.width() as f32, canvas.height() as f32)
                            };
                            if cw > 0.0 && ch > 0.0 {
                                if let Some(rid) = parts.tree.root_id() {
                                    if let Some(root_mut) = parts.tree.get_mut(rid) {
                                        let rf = root_mut.frame();
                                        if (rf.w - cw).abs() > 0.5 || (rf.h - ch).abs() > 0.5 {
                                            root_mut.set_frame(crate::core::Rect::new(
                                                0.0, 0.0, cw, ch,
                                            ));
                                            parts.tree.tree_version =
                                                parts.tree.tree_version.wrapping_add(1);
                                        }
                                    }
                                }
                            }
                        }
                        self.initial_size = (d.width, d.height);
                        self.window_visible = true;
                        parts.tree.mark_full_frame_dirty();
                    }
                }
            }
            UiEventType::WindowMaximize => {
                let info = platform.display().info(0);
                let w = info.bounds.w as i32;
                let h = info.bounds.h as i32;
                if w > 0 && h > 0 {
                    report_graphics_resize_error(
                        "secondary graphics maximize resize failed",
                        parts.engine.resize(w, h),
                    );
                    report_window_operation_error(
                        "secondary maximize resize_notify failed",
                        self._window.resize_notify(w, h),
                    );
                    self.window_visible = true;
                    parts.tree.mark_full_frame_dirty();
                }
            }
            UiEventType::WindowRestore => {
                let (w, h) = self.initial_size;
                report_graphics_resize_error(
                    "secondary graphics restore resize failed",
                    parts.engine.resize(w, h),
                );
                report_window_operation_error(
                    "secondary restore resize_notify failed",
                    self._window.resize_notify(w, h),
                );
                self.window_visible = true;
                parts.tree.mark_full_frame_dirty();
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
        let had_app_state_semantic_work = parts.tree.drain_app_state_semantic_events();

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

        had_main_thread_work || had_app_state_semantic_work
    }

    fn has_frame_work(&mut self, now: Instant) -> bool {
        if !self.rendered_first {
            return true;
        }

        let parts = self.session.parts_mut();
        parts
            .active_work
            .sync_app_timers(parts.app_timers.deadlines());
        if parts.tree.take_reconcile_requested() {
            *parts.reconcile_pending = true;
        }

        let due_registered_work = parts
            .active_work
            .next_deadline()
            .is_some_and(|deadline| deadline <= now);
        let has_layout_work = parts
            .tree
            .invalidation
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .has_layout();

        due_registered_work
            || !parts.main_thread_queue.is_empty()
            || parts.pending_root.is_some()
            || *parts.reconcile_pending
            || parts.tree.has_app_state_semantic_events()
            || parts.tree.has_pending_effects()
            || parts.tree.has_render_work()
            || has_layout_work
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
        let due_animation_ids = due_secondary_animation_ids(&due_work);
        let had_due_widget_timer_work =
            dispatch_due_secondary_active_work(parts.tree, &parts.app_timers, &due_work, clock);

        let mut main_thread_context =
            MainThreadContext::new(parts.pending_root, parts.reconcile_pending);
        let _had_main_thread_work = parts.main_thread_queue.drain(&mut main_thread_context);
        let had_app_state_semantic_work = parts.tree.drain_app_state_semantic_events();
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
        let pending_effects = parts.tree.has_pending_effects();
        let pending_render_work = parts.tree.has_render_work();
        let active_frame = had_registered_work
            || had_app_state_semantic_work
            || *parts.reconcile_pending
            || pending_effects
            || !self.rendered_first
            || pending_layout_work
            || pending_render_work;
        let discover_animation_work = had_due_widget_timer_work
            || had_app_state_semantic_work
            || *parts.reconcile_pending
            || pending_effects
            || !self.rendered_first
            || pending_layout_work
            || pending_render_work;

        if active_frame {
            let dt = (now - *last_frame).as_secs_f64().min(0.05);
            *last_frame = now;
            let animation_updates = update_due_and_discovered_secondary_animations(
                parts.tree,
                parts.active_work,
                &due_animation_ids,
                dt,
                discover_animation_work,
            );
            sync_secondary_animation_deadlines(parts.active_work, &animation_updates, now);
            if pending_effects {
                let _effects_ran = parts.tree.tick_effects();
            }
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
        let mut laid_out = false;
        if self.window_visible && (!self.rendered_first || has_layout_work) {
            let before_version = parts.tree.tree_version();
            parts.tree.layout();
            laid_out = true;
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
            // 本帧已 layout 则跳过重复首帧 layout（避免启动连跑 2–3 次）。
            if !laid_out {
                parts.tree.layout();
            }
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
            (frame_out.outcome, frame_out.inv_source)
        };

        let mut frame_committed = false;
        match outcome {
            RenderOutcome::Present(_) => {
                let _ = outcome_source;
                if engine_capabilities.uses_external_presenter() {
                    crate::core::log::error_fn(
                        "[Application] external secondary presenter reported final Present before platform submission",
                    );
                    self.rendered_first = false;
                } else {
                    self.rendered_first = true;
                    frame_committed = true;
                }
            }
            RenderOutcome::PresentPending(damage) => {
                let _ = outcome_source;
                if !engine_capabilities.uses_external_presenter() {
                    crate::core::log::error_fn(
                        "[Application] engine-managed secondary path returned external presentation pending",
                    );
                    self.rendered_first = false;
                } else {
                    let canvas = parts.engine.canvas_2d();
                    let cw = canvas.width();
                    let ch = canvas.height();
                    match self._window.presenter().present(
                        canvas.pixels_mut(),
                        cw,
                        ch,
                        damage.to_present_damage(),
                    ) {
                        Ok(()) => {
                            parts.engine.external_present_succeeded();
                            self.rendered_first = true;
                            frame_committed = true;
                        }
                        Err(error) => {
                            parts.engine.external_present_failed(error.clone());
                            crate::core::log::error_fn(format!(
                                "[Application] secondary external present failed: {}",
                                error.short_what()
                            ));
                            self.rendered_first = false;
                        }
                    }
                }
            }
            RenderOutcome::Idle => {}
            RenderOutcome::FrameReady(_) => {
                crate::core::log::error_fn(
                    "[Application] frame renderer returned FrameReady without final presentation",
                );
                self.rendered_first = false;
            }
            RenderOutcome::Failed(error) => {
                crate::core::log::error_fn(format!(
                    "[Application] secondary graphics frame failed: {}",
                    error.error().short_what()
                ));
                self.rendered_first = false;
            }
        }

        if frame_committed && self.deferred_show {
            if let Err(error) = self._window.show() {
                crate::core::log::error_fn(format!(
                    "secondary deferred show failed: {}",
                    error.short_what()
                ));
            } else {
                report_window_operation_error(
                    "secondary deferred raise failed",
                    self._window.raise(),
                );
                crate::core::log::info_fn("secondary first_present: window revealed after present");
            }
            self.deferred_show = false;
        }

        if self.window_visible
            && frame_committed
            && (has_layout_work || need_render)
        {
            // Failed external presentation must retain the invalidation for a retry.
            parts.tree.reset_invalidation();
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

    fn close(mut self) {
        self.session.shutdown();
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

    /// Select the GPU API once during window/engine initialization.
    pub fn graphics_backend(mut self, backend: GraphicsBackend) -> Self {
        self.graphics_backend = Some(backend);
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

    pub(crate) fn load_configured_settings(&mut self) -> bool {
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
        report_window_operation_error(
            "initial center_on_screen failed",
            platform_window.center_on_screen(),
        );
        let mut engine =
            match create_preferred_engine(platform_window.as_mut(), w, h, graphics_backend) {
                Some(engine) => engine,
                None => {
                    report_window_operation_error(
                        "initial engine failure cleanup close failed",
                        platform_window.close(),
                    );
                    return 1;
                }
            };
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
            move || wrap_root_with_notification_overlay(root_factory(), root_notifications.clone()),
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

        session.shutdown();
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

pub(crate) fn resolve_graphics_backend(
    builder: Option<GraphicsBackend>,
    env_value: Option<&str>,
    settings: Option<&SettingsService>,
) -> GraphicsBackend {
    if let Some(backend) = builder {
        return backend;
    }

    if let Some(value) = env_value {
        if let Some(backend) = parse_graphics_backend_config(GRAPHICS_BACKEND_ENV, value) {
            return backend;
        }
    }

    if let Some(settings) = settings {
        for key in GRAPHICS_BACKEND_SETTING_KEYS {
            if let Some(value) = settings.get(key) {
                if let Some(backend) = parse_graphics_backend_config(key, value) {
                    return backend;
                }
            }
        }
    }

    GraphicsBackend::Vulkan
}

fn parse_graphics_backend_config(source: &str, value: &str) -> Option<GraphicsBackend> {
    match value.parse::<GraphicsBackend>() {
        Ok(backend) => Some(backend),
        Err(err) => {
            crate::core::log::warn_fn(format!(
                "graphics backend config {source} ignored: {}",
                err.short_what()
            ));
            None
        }
    }
}

pub(crate) fn drain_pending_open_windows(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    drain_pending_open_windows_with_backend(
        platform,
        runtime,
        app_state,
        container,
        GraphicsBackend::Auto,
        on_window_start,
        secondary_windows,
    )
}

pub(crate) fn drain_pending_open_windows_with_backend(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: GraphicsBackend,
    on_window_start: Option<&Arc<dyn Fn(AppHandle) + Send + Sync>>,
    secondary_windows: &mut Vec<SecondaryWindowSession>,
) -> usize {
    let mut created = 0;
    while let Some(request) = runtime.take_next_open_window() {
        if let Some(mut window) = create_secondary_window(
            platform,
            runtime,
            app_state,
            container,
            graphics_backend,
            request,
        ) {
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

pub(crate) fn drain_secondary_window_queues(secondary_windows: &mut [SecondaryWindowSession]) -> bool {
    let mut drained = false;
    for window in secondary_windows {
        drained |= window.drain_main_thread_work();
    }
    drained
}

pub(crate) fn drain_secondary_window_frames(
    secondary_windows: &mut [SecondaryWindowSession],
    font_service: &FontService,
    image_service: &ImageService,
    theme: &RefCell<Theme>,
    debug_mode: &Cell<bool>,
    cursor_pos: &Cell<Point>,
    clock: &dyn AppClock,
) -> bool {
    let mut drained = false;
    let now = clock.now();
    for window in secondary_windows {
        if window.has_frame_work(now) {
            drained |= window.drain_frame(
                font_service,
                image_service,
                theme,
                debug_mode,
                cursor_pos,
                clock,
            );
        }
    }
    drained
}

pub(crate) fn secondary_windows_next_deadline(
    secondary_windows: &mut [SecondaryWindowSession],
) -> Option<Instant> {
    secondary_windows
        .iter_mut()
        .filter_map(SecondaryWindowSession::next_deadline)
        .min()
}

pub(crate) fn dispatch_secondary_window_event(
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
        secondary_windows.remove(index).close();
        true
    }
}

pub(crate) fn dispatch_secondary_system_theme_changed(
    secondary_windows: &mut [SecondaryWindowSession],
    is_dark: bool,
) -> bool {
    let mut dispatched = false;
    for window in secondary_windows {
        window
            .session
            .parts_mut()
            .tree
            .dispatch_event(&SystemEvent::ThemeChanged { is_dark });
        dispatched = true;
    }
    dispatched
}

pub(crate) fn apply_runtime_theme_change(
    theme: &RefCell<Theme>,
    root_tree: &mut WidgetTree,
    secondary_windows: &mut [SecondaryWindowSession],
    next_theme: Theme,
) {
    let is_dark = next_theme.is_dark();
    *theme.borrow_mut() = next_theme;
    root_tree.dispatch_event(&SystemEvent::ThemeChanged { is_dark });
    dispatch_secondary_system_theme_changed(secondary_windows, is_dark);
}

fn create_secondary_window(
    platform: &mut dyn Platform,
    runtime: &AppRuntime,
    app_state: &AppState,
    container: &Container,
    graphics_backend: GraphicsBackend,
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
        report_window_operation_error(
            "window_id mismatch cleanup close failed",
            platform_window.close(),
        );
        runtime.close_session(window_id);
        crate::core::log::error_fn(format!(
            "open_window window_id mismatch: reserved={}, native={}",
            window_id.raw(),
            actual.raw()
        ));
        return None;
    }

    report_window_operation_error(
        "secondary center_on_screen failed",
        platform_window.center_on_screen(),
    );
    let mut engine =
        match create_preferred_engine(platform_window.as_mut(), width, height, graphics_backend) {
            Some(engine) => engine,
            None => {
                report_window_operation_error(
                    "secondary engine failure cleanup close failed",
                    platform_window.close(),
                );
                runtime.close_session(window_id);
                return None;
            }
        };
    // 与主窗一致：首帧 present 成功后再 show，避免空窗白屏。

    let notifications = container.resolve_clone::<AppNotificationState>();
    let wrapped_root = move || {
        let root_node = root();
        match notifications.clone() {
            Some(state) => wrap_root_with_notification_overlay(root_node, state),
            None => root_node,
        }
    };
    let mut session =
        WindowSession::from_root_factory_for_window(window_id, wrapped_root, engine, width, height);
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
        session,
        _window: platform_window,
        handle,
        frame_renderer: FrameRenderer::new(),
        rendered_first: false,
        last_frame: None,
        window_visible: true,
        initial_size: (width, height),
        deferred_show: true,
    })
}

fn dispatch_due_secondary_active_work(
    tree: &mut WidgetTree,
    app_timers: &AppTimerQueue,
    due_work: &[ActiveWorkKind],
    clock: &dyn AppClock,
) -> bool {
    let mut handled_widget_timer = false;
    for work in due_work {
        match *work {
            ActiveWorkKind::Timer(id) => {
                handled_widget_timer |= tree.dispatch_timer_work(id) == EventResult::Handled;
            }
            ActiveWorkKind::AppTimer(id) => {
                app_timers.fire(id, clock.now());
            }
            _ => {}
        }
    }
    handled_widget_timer
}

fn due_secondary_animation_ids(due_work: &[ActiveWorkKind]) -> Vec<NodeId> {
    due_work
        .iter()
        .filter_map(|work| match *work {
            ActiveWorkKind::Animation(id) => Some(id),
            _ => None,
        })
        .collect()
}

fn update_due_and_discovered_secondary_animations(
    tree: &mut WidgetTree,
    active_work: &ActiveWorkRegistry,
    due_animation_ids: &[NodeId],
    dt: f64,
    discover_animation_work: bool,
) -> Vec<(NodeId, bool)> {
    let mut updates = if due_animation_ids.is_empty() {
        Vec::new()
    } else {
        tree.update_animation_nodes(due_animation_ids.iter().copied(), dt)
    };

    if discover_animation_work {
        let mut registered_ids: Vec<_> = active_work.animation_ids().collect();
        registered_ids.extend_from_slice(due_animation_ids);
        updates.extend(tree.update_animations_except(registered_ids, dt));
    }

    updates
}

fn sync_secondary_animation_deadlines(
    active_work: &mut ActiveWorkRegistry,
    animation_updates: &[(NodeId, bool)],
    now: Instant,
) {
    for &(id, animating) in animation_updates {
        let kind = ActiveWorkKind::Animation(id);
        if animating {
            active_work.register(kind, now + Duration::from_millis(16));
        } else {
            active_work.unregister(kind);
        }
    }
}

fn sync_secondary_root_frame_to_engine(tree: &mut WidgetTree, engine: &mut dyn GraphicsEngine) {
    let (ew, eh) = {
        let canvas = engine.canvas_2d();
        (canvas.width() as f32, canvas.height() as f32)
    };
    if ew <= 0.0 || eh <= 0.0 {
        return;
    }
    // 与主窗 sync_root_frame_to_engine 同策略：bootstrap 或引擎已更大时对齐根 frame。
    let need_sync = tree
        .root_id()
        .and_then(|rid| tree.get(rid))
        .is_some_and(|root| {
            let rf = root.frame();
            let bootstrap = rf.w <= 1.0 || rf.h <= 1.0;
            let engine_larger = ew > rf.w + 0.5 || eh > rf.h + 0.5;
            bootstrap || engine_larger
        });
    if need_sync {
        if let Some(rid) = tree.root_id() {
            if let Some(root_mut) = tree.get_mut(rid) {
                root_mut.set_frame(crate::core::Rect::new(0.0, 0.0, ew, eh));
            }
        }
        tree.tree_version = tree.tree_version.wrapping_add(1);
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

fn recreate_exact_graphics_recipe(
    surface: NativeSurfaceHandle,
    width: i32,
    height: i32,
    recipe: GraphicsRecipe,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    let context = try_create_gpu_recipe(recipe, surface, width, height)?;
    assemble_graphics_engine(context, width, height).map_err(|failure| failure.into_error())
}

fn create_software_recovery_engine(
    width: i32,
    height: i32,
) -> Result<Box<dyn GraphicsEngine>, Error> {
    let mut engine = SoftwareEngine::new();
    engine.initialize(width, height)?;
    Ok(Box::new(engine))
}

pub(crate) fn graphics_recovery_rebuilder(
    surface: NativeSurfaceHandle,
    requested: GraphicsBackend,
    selected_recipe: GraphicsRecipe,
) -> GraphicsEngineRebuilder {
    let candidates = gpu_recipe_candidates(requested);
    let mut current_recipe = selected_recipe;
    Box::new(move |action, width, height| match action {
        RecoveryAction::RebuildSurface | RecoveryAction::RebuildRecipe => {
            recreate_exact_graphics_recipe(surface, width, height, current_recipe)
        }
        RecoveryAction::TryNextRecipe => {
            let start = candidates
                .iter()
                .position(|recipe| *recipe == current_recipe)
                .map(|index| index + 1)
                .unwrap_or(0);
            let mut last_error = Error::new(
                Errc::PlatformError,
                format!("graphics recovery: no next recipe after {current_recipe}"),
            );
            for candidate in candidates.iter().copied().skip(start) {
                match recreate_exact_graphics_recipe(surface, width, height, candidate) {
                    Ok(engine) => {
                        current_recipe = candidate;
                        return Ok(engine);
                    }
                    Err(error) => last_error = error,
                }
            }
            Err(last_error)
        }
        RecoveryAction::UseSoftware => create_software_recovery_engine(width, height),
        RecoveryAction::Abort | RecoveryAction::AbortOutOfMemory => Err(Error::new(
            Errc::InvalidState,
            format!("graphics recovery: forbidden action {action:?}"),
        )),
    })
}

fn create_preferred_engine(
    platform_window: &mut dyn PlatformWindow,
    width: i32,
    height: i32,
    graphics_backend: GraphicsBackend,
) -> Option<Box<dyn GraphicsEngine>> {
    // SAFETY: PlatformWindow owns this surface for the complete synchronous
    // window session. Graphics bootstrap/recovery execute on the same event
    // loop thread, and `NativeSurfaceHandle` is !Send + !Sync.
    let surface = unsafe { NativeSurfaceHandle::from_raw(platform_window.native_surface_ptr()) };
    match bootstrap_graphics_engine(surface, width, height, graphics_backend) {
        Ok(gpu) => {
            if gpu.report.failures.is_empty() {
                crate::core::log::info_fn(format!("GPU engine initialized ({})", gpu.selected));
            } else {
                crate::core::log::warn_fn(format!(
                    "GPU engine initialized after probe fallback; selected={}; failures=[{}]",
                    gpu.selected,
                    format_probe_failures(&gpu.report)
                ));
            }
            let engine = RecoveringGraphicsEngine::new(
                gpu.engine,
                graphics_recovery_rebuilder(surface, graphics_backend, gpu.selected_recipe),
            )
            .with_extent(width, height);
            Some(Box::new(engine))
        }
        Err(report) => {
            crate::core::log::warn_fn(format_gpu_probe_fallback(graphics_backend, &report));

            let mut engine = SoftwareEngine::new();
            match engine.initialize(width, height) {
                Ok(()) => {
                    crate::core::log::info_fn("CPU software engine initialized");
                    Some(Box::new(engine))
                }
                Err(e) => {
                    let _ = engine.try_shutdown();
                    crate::core::log::error_fn(format!("SoftwareEngine 初始化失败: {}", e.what()));
                    None
                }
            }
        }
    }
}

fn format_probe_failures(report: &ProbeReport) -> String {
    if report.failures.is_empty() {
        return "none".to_string();
    }

    report
        .failures
        .iter()
        .enumerate()
        .map(|(index, failure)| {
            format!(
                "failure[{index}]={{candidate={}, detail={:?}}}",
                failure.backend, failure.message
            )
        })
        .collect::<Vec<_>>()
        .join("; ")
}

pub(crate) fn format_gpu_probe_fallback(request: GraphicsBackend, report: &ProbeReport) -> String {
    format!(
        "GPU probe exhausted; request={request}; platform={}; fallback=software_cpu; failures=[{}]",
        graphics_runtime_platform(),
        format_probe_failures(report)
    )
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

