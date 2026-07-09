use super::*;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::test_clock::system_clock;
use crate::app::window_session::WindowLoopState;
use crate::core::{Point, Rect};
use crate::data::SettingsService;
use crate::draw::engine::bootstrap::ProbeFailure;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::native::test_harness::FakePlatform;
use crate::native::traits::event::{
    ClipboardData, FileDropData, ImeCompositionData, LocaleChangeData, ThemeChangeData,
};
use crate::native::traits::present::GraphicsBackend;
use crate::ui::state::State;
use crate::ui::theme::Theme;
use crate::ui::view::combinators::{dynamic_label, label};
use crate::ui::view::ViewNode;
use crate::ui::widgets::Label;
use crate::ui::{
    EventHandler, EventResult, HandlerRegistration, SemanticEvent, SemanticKind, SystemEvent,
    WidgetAnimation, WidgetCapabilities, WidgetComponent, WidgetRender, WidgetTree,
};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

struct RecordingWidget {
    focus_events: Arc<AtomicBool>,
}

impl RecordingWidget {
    fn new(focus_events: Arc<AtomicBool>) -> Self {
        Self { focus_events }
    }
}

impl WidgetComponent for RecordingWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::EVENT)
    }

    fn as_event(&self) -> Option<&dyn EventHandler> {
        Some(self)
    }

    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        Some(self)
    }
}

impl EventHandler for RecordingWidget {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if matches!(event, SystemEvent::WindowFocus) {
            self.focus_events.store(true, Ordering::Relaxed);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }
}

struct ThemeRecordingWidget {
    theme_events: Arc<AtomicUsize>,
}

impl ThemeRecordingWidget {
    fn new(theme_events: Arc<AtomicUsize>) -> Self {
        Self { theme_events }
    }
}

impl WidgetComponent for ThemeRecordingWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::EVENT)
    }

    fn as_event(&self) -> Option<&dyn EventHandler> {
        Some(self)
    }

    fn as_event_mut(&mut self) -> Option<&mut dyn EventHandler> {
        Some(self)
    }
}

impl EventHandler for ThemeRecordingWidget {
    fn on_event(&mut self, event: &SystemEvent) -> EventResult {
        if matches!(event, SystemEvent::ThemeChanged { .. }) {
            self.theme_events.fetch_add(1, Ordering::Relaxed);
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }
}

struct PaletteWidget;

impl WidgetComponent for PaletteWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::RENDER)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }
}

impl WidgetRender for PaletteWidget {
    fn render(
        &self,
        _frame: Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &WidgetTree,
    ) {
    }
}

struct CountingAnimationWidget {
    update_calls: Arc<AtomicUsize>,
}

impl CountingAnimationWidget {
    fn new(update_calls: Arc<AtomicUsize>) -> Self {
        Self { update_calls }
    }
}

impl WidgetComponent for CountingAnimationWidget {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn into_any(self: Box<Self>) -> Box<dyn Any> {
        self
    }

    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(WidgetCapabilities::RENDER | WidgetCapabilities::ANIMATION)
    }

    fn as_render(&self) -> Option<&dyn WidgetRender> {
        Some(self)
    }

    fn as_render_mut(&mut self) -> Option<&mut dyn WidgetRender> {
        Some(self)
    }

    fn as_animation(&self) -> Option<&dyn WidgetAnimation> {
        Some(self)
    }

    fn as_animation_mut(&mut self) -> Option<&mut dyn WidgetAnimation> {
        Some(self)
    }
}

impl WidgetRender for CountingAnimationWidget {
    fn render(
        &self,
        _frame: Rect,
        _ctx: &mut crate::draw::painting::PaintContext,
        _tree: &WidgetTree,
    ) {
    }
}

impl WidgetAnimation for CountingAnimationWidget {
    fn update_animation(&mut self, _dt: f64) -> bool {
        self.update_calls.fetch_add(1, Ordering::Relaxed);
        false
    }
}

#[test]
fn app_default_does_not_follow_system_theme() {
    assert!(!App::default().follow_system_theme);
}

#[test]
fn app_builder_sets_follow_system_theme() {
    assert!(App::new().follow_system_theme(true).follow_system_theme);
}

#[test]
fn app_builder_sets_graphics_backend() {
    let app = App::new().graphics_backend(GraphicsBackend::Vulkan);

    assert_eq!(app.graphics_backend, Some(GraphicsBackend::Vulkan));
    assert_eq!(app.configured_graphics_backend(), GraphicsBackend::Vulkan);
}

#[test]
fn app_graphics_backend_defaults_to_auto() {
    assert_eq!(
        resolve_graphics_backend(None, None, None),
        GraphicsBackend::Auto
    );
}

#[test]
fn app_graphics_backend_prefers_builder_over_env_and_settings() {
    let mut settings = SettingsService::new();
    settings.set("graphics_backend", "opengles");

    assert_eq!(
        resolve_graphics_backend(
            Some(GraphicsBackend::D3d11),
            Some("vulkan"),
            Some(&settings)
        ),
        GraphicsBackend::D3d11
    );
}

#[test]
fn app_graphics_backend_uses_env_before_settings() {
    let mut settings = SettingsService::new();
    settings.set("graphics_backend", "opengles");

    assert_eq!(
        resolve_graphics_backend(None, Some("vulkan"), Some(&settings)),
        GraphicsBackend::Vulkan
    );
}

#[test]
fn app_graphics_backend_falls_back_from_invalid_env_to_settings() {
    let mut settings = SettingsService::new();
    settings.set("uix.graphics_backend", "gles");

    assert_eq!(
        resolve_graphics_backend(None, Some("not-a-backend"), Some(&settings)),
        GraphicsBackend::OpenGlEs
    );
}

#[test]
fn gpu_probe_fallback_format_preserves_request_failures_and_order() {
    let report = ProbeReport {
        failures: vec![
            ProbeFailure {
                backend: GraphicsBackend::D3d11,
                message: "stage=context_create; selected=none; error=device unavailable".into(),
            },
            ProbeFailure {
                backend: GraphicsBackend::OpenGlEs,
                message: "stage=engine_initialize; selected=opengles; error=shader failed".into(),
            },
        ],
    };

    let message = format_gpu_probe_fallback(GraphicsBackend::Auto, &report);
    assert!(message.contains("request=auto"));
    assert!(message.contains("falling_back=cpu"));
    let d3d11 = message.find("candidate=d3d11");
    let opengles = message.find("candidate=opengles");
    assert!(matches!(
        (d3d11, opengles),
        (Some(d3d11), Some(opengles)) if d3d11 < opengles
    ));
    assert!(message.contains("stage=context_create"));
    assert!(message.contains("device unavailable"));
    assert!(message.contains("stage=engine_initialize"));
    assert!(message.contains("shader failed"));
}

#[test]
fn app_cli_mode_without_cli_returns_error() {
    let mut app = App::new().mode(AppMode::CLI);

    assert_eq!(app.run_cli(), 1);
    assert_eq!(app.exit_code(), 1);
}

#[test]
fn app_settings_is_opt_in() {
    let mut app = App::new();

    assert!(app.load_configured_settings());
    assert!(!app.container().has::<SettingsService>());
}

#[test]
fn app_settings_load_registers_settings_service() {
    let path = std::env::temp_dir().join(format!(
        "uix-settings-{}-{}.json",
        std::process::id(),
        "app-settings-load"
    ));
    std::fs::write(
        &path,
        r##"{"theme_mode":"dark","brand_primary":"#1677ff"}"##,
    )
    .unwrap();

    let mut app = App::new().settings(path.to_string_lossy().to_string());

    assert!(app.load_configured_settings());
    let settings = app
        .container()
        .resolve::<SettingsService>()
        .expect("settings service");
    assert_eq!(
        settings.loaded_path(),
        Some(path.to_string_lossy().as_ref())
    );
    assert_eq!(settings.get("theme_mode"), Some("dark"));
    assert_eq!(settings.get("brand_primary"), Some("#1677ff"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn app_run_after_returns_cancelable_timer_handle() {
    let app = App::new();
    let handle = app.run_after(std::time::Duration::from_secs(1), || {});

    assert_eq!(app.app_timers.len(), 1);
    handle.cancel();
    assert_eq!(app.app_timers.len(), 0);
}

#[test]
fn app_run_interval_is_canceled_on_handle_drop() {
    let app = App::new();
    let handle = app.run_interval(std::time::Duration::from_secs(1), || {});

    assert_eq!(app.app_timers.len(), 1);
    drop(handle);
    assert_eq!(app.app_timers.len(), 0);
}

#[test]
fn app_post_to_ui_enqueues_main_thread_job() {
    let app = App::new();

    app.post_to_ui(|| {});

    assert_eq!(app.main_thread_queue.len(), 1);
}

#[test]
fn app_on_start_stores_start_callback() {
    let app = App::new().on_start(|_| {});

    assert!(app.on_start.is_some());
}

#[test]
fn app_on_start_accepts_fn_once_callback() {
    struct StartupToken(String);
    let token = StartupToken(String::from("boot"));

    let app = App::new().on_start(move |_| {
        let StartupToken(value) = token;
        assert_eq!(value, "boot");
    });

    assert!(app.on_start.is_some());
}

#[test]
fn app_on_window_start_stores_secondary_window_callback() {
    let app = App::new().on_window_start(|_| {});

    assert!(app.on_window_start.is_some());
}

#[test]
fn app_handle_uses_root_window_and_shared_queues() {
    let app = App::new();
    let handle = app.app_handle();

    assert_eq!(handle.window_id(), WindowId::root());
    assert!(std::sync::Arc::ptr_eq(
        &app.app_state().inner,
        &handle.app_state().inner
    ));
    handle.post_to_ui(|| {});
    let _timer = handle.run_after(std::time::Duration::from_secs(1), || {});

    assert_eq!(app.main_thread_queue.len(), 1);
    assert_eq!(app.app_timers.len(), 1);
}

#[test]
fn app_handle_resolve_reads_builder_singleton_clone() {
    let app = App::new().singleton(String::from("service"));
    let handle = app.app_handle();

    assert_eq!(handle.resolve::<String>(), Some(String::from("service")));
}

#[test]
fn app_handle_can_target_registered_platform_window_id() {
    let app = App::new();
    let handle = app.app_handle_for_window(WindowId::new(12));

    assert_eq!(handle.window_id(), WindowId::new(12));
}

#[test]
fn drain_pending_open_windows_bootstraps_secondary_session() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let request =
        runtime.request_open_window(WindowConfig::new("Inspector", 320, 600, || label("child")));
    let started = Arc::new(Mutex::new(Vec::new()));
    let on_window_start: Arc<dyn Fn(AppHandle) + Send + Sync> = {
        let started = started.clone();
        Arc::new(move |handle| {
            started
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push(handle.window_id());
        })
    };
    let mut secondary_windows = Vec::new();

    let created = drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        Some(&on_window_start),
        &mut secondary_windows,
    );

    assert_eq!(created, 1);
    assert_eq!(platform.window_manager.create_calls.len(), 2);
    assert_eq!(
        platform.window_manager.create_calls[1],
        ("Inspector".to_string(), 320, 600)
    );
    assert_eq!(
        *started.lock().unwrap_or_else(|e| e.into_inner()),
        vec![request.window_id]
    );
    assert_eq!(secondary_windows[0].handle.window_id(), request.window_id);
    assert_eq!(secondary_windows[0].session.window_id(), request.window_id);
    assert_ne!(secondary_windows[0].handle.window_id(), WindowId::ROOT);
    assert!(request.alive.load(Ordering::Acquire));

    secondary_windows[0]
        .handle
        .update_view(|| label("updated child"));
    assert!(drain_secondary_window_queues(&mut secondary_windows));
    let (tree, _) = secondary_windows[0].session.tree_and_engine_mut();
    let text = tree
        .root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap()
        .text();
    assert_eq!(text, "updated child");
}

#[test]
fn drain_secondary_window_queues_drains_all_sessions() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let _first = runtime.request_open_window(WindowConfig::new("A", 320, 240, || label("a")));
    let _second = runtime.request_open_window(WindowConfig::new("B", 320, 240, || label("b")));
    let mut secondary_windows = Vec::new();
    assert_eq!(
        drain_pending_open_windows(
            &mut platform,
            &runtime,
            &AppState::new(),
            &Container::new(),
            None,
            &mut secondary_windows,
        ),
        2
    );

    secondary_windows[0]
        .handle
        .update_view(|| label("updated a"));
    secondary_windows[1]
        .handle
        .update_view(|| label("updated b"));

    assert!(drain_secondary_window_queues(&mut secondary_windows));
    let (first_tree, _) = secondary_windows[0].session.tree_and_engine_mut();
    let first_text = first_tree
        .root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap()
        .text();
    assert_eq!(first_text, "updated a");
    let (second_tree, _) = secondary_windows[1].session.tree_and_engine_mut();
    let second_text = second_tree
        .root()
        .unwrap()
        .component()
        .as_any()
        .downcast_ref::<Label>()
        .unwrap()
        .text();
    assert_eq!(second_text, "updated b");
}

#[test]
fn dispatch_secondary_window_event_routes_by_window_id() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let child_focus = Arc::new(AtomicBool::new(false));
    let child = runtime.request_open_window(WindowConfig::new("Child", 320, 240, {
        let child_focus = child_focus.clone();
        move || ViewNode::leaf(RecordingWidget::new(child_focus.clone()))
    }));
    let mut secondary_windows = Vec::new();
    assert_eq!(
        drain_pending_open_windows(
            &mut platform,
            &runtime,
            &AppState::new(),
            &Container::new(),
            None,
            &mut secondary_windows,
        ),
        1
    );

    assert!(dispatch_secondary_window_event(
        &mut secondary_windows,
        &mut platform,
        &UiEvent::new(UiEventType::WindowFocus, UiEventPayload::None).for_window(child.window_id),
    ));

    assert!(child_focus.load(Ordering::Relaxed));
}

#[test]
fn dispatch_secondary_system_theme_changed_broadcasts_to_all_sessions() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let first_theme_events = Arc::new(AtomicUsize::new(0));
    let second_theme_events = Arc::new(AtomicUsize::new(0));
    runtime.request_open_window(WindowConfig::new("First", 320, 240, {
        let first_theme_events = first_theme_events.clone();
        move || ViewNode::leaf(ThemeRecordingWidget::new(first_theme_events.clone()))
    }));
    runtime.request_open_window(WindowConfig::new("Second", 320, 240, {
        let second_theme_events = second_theme_events.clone();
        move || ViewNode::leaf(ThemeRecordingWidget::new(second_theme_events.clone()))
    }));
    let mut secondary_windows = Vec::new();
    assert_eq!(
        drain_pending_open_windows(
            &mut platform,
            &runtime,
            &AppState::new(),
            &Container::new(),
            None,
            &mut secondary_windows,
        ),
        2
    );

    assert!(dispatch_secondary_system_theme_changed(
        &mut secondary_windows,
        true
    ));

    assert_eq!(first_theme_events.load(Ordering::Relaxed), 1);
    assert_eq!(second_theme_events.load(Ordering::Relaxed), 1);
}

#[test]
fn dispatch_secondary_system_theme_changed_invalidates_palette_widgets() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    runtime.request_open_window(WindowConfig::new("Child", 320, 240, || {
        ViewNode::leaf(PaletteWidget)
    }));
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    {
        let parts = secondary_windows[0].session.parts_mut();
        parts.tree.reset_invalidation();
        assert!(!parts.tree.has_render_work());
    }

    assert!(dispatch_secondary_system_theme_changed(
        &mut secondary_windows,
        true
    ));

    let parts = secondary_windows[0].session.parts_mut();
    assert!(parts.tree.has_render_work());
}

#[test]
fn dispatch_secondary_system_theme_changed_reports_no_work_for_empty_list() {
    let mut secondary_windows = Vec::new();

    assert!(!dispatch_secondary_system_theme_changed(
        &mut secondary_windows,
        true
    ));
}

#[test]
fn dispatch_secondary_window_close_removes_session() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let child =
        runtime.request_open_window(WindowConfig::new("Child", 320, 240, || label("child")));
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    assert!(dispatch_secondary_window_event(
        &mut secondary_windows,
        &mut platform,
        &UiEvent::close().for_window(child.window_id),
    ));

    assert!(secondary_windows.is_empty());
    assert!(!child.alive.load(Ordering::Acquire));
}

#[test]
fn drain_secondary_window_frames_fires_window_timer() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let child =
        runtime.request_open_window(WindowConfig::new("Child", 320, 240, || label("child")));
    let fired = Arc::new(AtomicBool::new(false));
    let _timer = runtime.run_after(child.window_id, Duration::ZERO, {
        let fired = fired.clone();
        move || fired.store(true, Ordering::Relaxed)
    });
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));
    let clock = system_clock();

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert!(fired.load(Ordering::Relaxed));
}

#[test]
fn secondary_windows_next_deadline_reads_window_timers() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let child =
        runtime.request_open_window(WindowConfig::new("Child", 320, 240, || label("child")));
    let _timer = runtime.run_after(child.window_id, Duration::from_millis(10), || {});
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    assert!(secondary_windows_next_deadline(&mut secondary_windows).is_some());
}

#[test]
fn drain_secondary_window_frames_records_registered_active_state() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let child =
        runtime.request_open_window(WindowConfig::new("Child", 320, 240, || label("child")));
    let _timer = runtime.run_after(child.window_id, Duration::from_secs(1), || {});
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));
    let clock = system_clock();

    drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    );

    assert_eq!(
        secondary_windows[0].session.loop_state(),
        WindowLoopState::RegisteredActive
    );
}

#[test]
fn drain_secondary_window_frames_skips_deep_idle_windows() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    runtime.request_open_window(WindowConfig::new("Child", 320, 240, || label("child")));
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));
    let clock = system_clock();

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert_eq!(
        secondary_windows[0].session.loop_state(),
        WindowLoopState::DeepIdle
    );

    secondary_windows[0].last_frame = None;
    assert!(!drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert!(secondary_windows[0].last_frame.is_none());
}

#[test]
fn state_set_only_wakes_secondary_windows_bound_to_that_state() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let state = State::new(0);
    runtime.request_open_window(WindowConfig::new("First", 320, 240, {
        let state = state.clone();
        move || {
            let state = state.clone();
            dynamic_label(move || format!("first {}", state.get()))
        }
    }));
    runtime.request_open_window(WindowConfig::new("Second", 320, 240, {
        let state = state.clone();
        move || {
            let state = state.clone();
            dynamic_label(move || format!("second {}", state.get()))
        }
    }));
    runtime.request_open_window(WindowConfig::new("Idle", 320, 240, || label("idle")));
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));
    let clock = system_clock();

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert_eq!(secondary_windows.len(), 3);
    assert!(secondary_windows.iter().all(|window| {
        window.last_frame.is_some() && window.session.loop_state() == WindowLoopState::DeepIdle
    }));

    for window in &mut secondary_windows {
        window.last_frame = None;
    }
    state.set(1);

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert!(secondary_windows[0].last_frame.is_some());
    assert!(secondary_windows[1].last_frame.is_some());
    assert!(secondary_windows[2].last_frame.is_none());
    assert_eq!(
        secondary_windows[2].session.loop_state(),
        WindowLoopState::DeepIdle
    );
}

#[test]
fn lookup_emit_only_wakes_secondary_window_containing_target() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    runtime.request_open_window(WindowConfig::new("Unrelated", 320, 240, || {
        label("unrelated")
    }));

    let calls = Arc::new(Mutex::new(Vec::new()));
    runtime.request_open_window(WindowConfig::new("Target", 320, 240, {
        let calls = calls.clone();
        move || {
            let mut node = label("target");
            let calls = calls.clone();
            node.handlers.push(HandlerRegistration::new(
                SemanticKind::Change,
                Box::new(move |event| {
                    calls
                        .lock()
                        .unwrap_or_else(|e| e.into_inner())
                        .push((event.target, event.current_target));
                }),
            ));
            node
        }
    }));

    let app_state = AppState::new();
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &app_state,
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));
    let clock = system_clock();

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    let target_id = secondary_windows[1]
        .session
        .tree_and_engine_mut()
        .0
        .root_id()
        .expect("target root should exist");
    for window in &mut secondary_windows {
        window.last_frame = None;
    }

    let handle = app_state
        .get_handle(target_id)
        .expect("target should be registered in shared AppState");
    assert_eq!(
        handle.emit(SemanticEvent::change(target_id, "from-lookup")),
        EventResult::Handled
    );

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert!(secondary_windows[0].last_frame.is_none());
    assert!(secondary_windows[1].last_frame.is_some());
    assert_eq!(
        calls.lock().unwrap_or_else(|e| e.into_inner()).as_slice(),
        &[(target_id, target_id)]
    );
}

#[test]
fn noop_secondary_post_to_ui_does_not_tick_animation() {
    let mut platform = FakePlatform::new();
    let _root_window = platform
        .window_manager()
        .create_window("Root", 800, 600)
        .unwrap();
    let runtime = AppRuntime::new();
    runtime.register_session(
        WindowId::new(1),
        AppTimerQueue::new(),
        MainThreadQueue::new(),
        Arc::new(AtomicBool::new(true)),
    );
    let update_calls = Arc::new(AtomicUsize::new(0));
    runtime.request_open_window(WindowConfig::new("Child", 320, 240, {
        let update_calls = update_calls.clone();
        move || ViewNode::leaf(CountingAnimationWidget::new(update_calls.clone()))
    }));
    let mut secondary_windows = Vec::new();
    drain_pending_open_windows(
        &mut platform,
        &runtime,
        &AppState::new(),
        &Container::new(),
        None,
        &mut secondary_windows,
    );

    let font_service = FontService::new();
    let image_service = ImageService::new();
    let theme = RefCell::new(Theme::default());
    let debug_mode = Cell::new(false);
    let cursor_pos = Cell::new(Point::new(0.0, 0.0));
    let clock = system_clock();

    assert!(drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert_eq!(update_calls.load(Ordering::Relaxed), 1);

    secondary_windows[0].handle.post_to_ui(|| {});
    assert!(!drain_secondary_window_frames(
        &mut secondary_windows,
        &font_service,
        &image_service,
        &theme,
        &debug_mode,
        &cursor_pos,
        clock.as_ref(),
    ));
    assert_eq!(update_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn app_handle_resolves_builder_singletons_at_runtime() {
    let app = App::new().singleton("runtime-config".to_string());
    let handle = app.app_handle();

    assert_eq!(
        handle.resolve::<String>(),
        Some("runtime-config".to_string())
    );
}

#[test]
fn app_handle_resolves_loaded_settings_service_at_runtime() {
    let path = std::env::temp_dir().join(format!(
        "uix-settings-{}-{}.json",
        std::process::id(),
        "app-handle-resolve"
    ));
    std::fs::write(&path, r##"{"theme_mode":"dark"}"##).unwrap();

    let mut app = App::new().settings(path.to_string_lossy().to_string());

    assert!(app.load_configured_settings());
    let handle = app.app_handle();
    let settings = handle
        .resolve::<SettingsService>()
        .expect("settings service");
    assert_eq!(
        settings.loaded_path(),
        Some(path.to_string_lossy().as_ref())
    );
    assert_eq!(settings.get("theme_mode"), Some("dark"));

    let _ = std::fs::remove_file(path);
}

#[test]
fn map_theme_changed_event() {
    let event = UiEvent::new(
        UiEventType::ThemeChanged,
        UiEventPayload::ThemeChanged(ThemeChangeData { is_dark: true }),
    );

    assert!(matches!(
        map_ui_event(&event),
        Some(SystemEvent::ThemeChanged { is_dark: true })
    ));
}

#[test]
fn map_locale_changed_event() {
    let event = UiEvent::new(
        UiEventType::LocaleChanged,
        UiEventPayload::LocaleChanged(LocaleChangeData {
            locale: "zh-CN".to_string(),
        }),
    );

    assert!(matches!(
        map_ui_event(&event),
        Some(SystemEvent::LocaleChanged { locale }) if locale == "zh-CN"
    ));
}

#[test]
fn map_file_drop_event() {
    let event = UiEvent::new(
        UiEventType::FileDrop,
        UiEventPayload::FileDrop(FileDropData {
            files: vec!["a.txt".to_string(), "nested/b.png".to_string()],
            position: Point::new(12.0, 34.0),
        }),
    );

    assert!(matches!(
        map_ui_event(&event),
        Some(SystemEvent::FileDrop { files, position })
            if files == vec!["a.txt".to_string(), "nested/b.png".to_string()]
                && position == Point::new(12.0, 34.0)
    ));
}

#[test]
fn map_clipboard_events() {
    assert!(matches!(
        map_ui_event(&UiEvent::copy()),
        Some(SystemEvent::Copy)
    ));
    assert!(matches!(
        map_ui_event(&UiEvent::cut()),
        Some(SystemEvent::Cut)
    ));

    let paste = UiEvent::new(
        UiEventType::Paste,
        UiEventPayload::Clipboard(ClipboardData {
            text: "hello".to_string(),
        }),
    );
    assert!(matches!(
        map_ui_event(&paste),
        Some(SystemEvent::Paste { text }) if text == "hello"
    ));
}

#[test]
fn map_ime_composition_events() {
    assert!(matches!(
        map_ui_event(&UiEvent::ime_composition_start()),
        Some(SystemEvent::ImeCompositionStart)
    ));

    let update = UiEvent::new(
        UiEventType::ImeCompositionUpdate,
        UiEventPayload::ImeComposition(ImeCompositionData {
            text: "zh".to_string(),
        }),
    );
    assert!(matches!(
        map_ui_event(&update),
        Some(SystemEvent::ImeCompositionUpdate { text }) if text == "zh"
    ));

    let end = UiEvent::new(
        UiEventType::ImeCompositionEnd,
        UiEventPayload::ImeComposition(ImeCompositionData {
            text: "中".to_string(),
        }),
    );
    assert!(matches!(
        map_ui_event(&end),
        Some(SystemEvent::ImeCompositionEnd { text }) if text == "中"
    ));
}
