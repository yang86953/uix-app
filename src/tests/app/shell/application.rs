use super::*;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::app::test_clock::system_clock;
use crate::app::window_session::WindowLoopState;
use crate::core::Point;
use crate::data::SettingsService;
use crate::draw::font::font_service::FontService;
use crate::draw::image::ImageService;
use crate::native::test_harness::FakePlatform;
use crate::native::traits::event::{
    ClipboardData, ImeCompositionData, LocaleChangeData, ThemeChangeData,
};
use crate::ui::theme::Theme;
use crate::ui::view::combinators::label;
use crate::ui::view::ViewNode;
use crate::ui::widgets::Label;
use crate::ui::{EventHandler, EventResult, SystemEvent, WidgetCapabilities, WidgetComponent};
use std::any::Any;
use std::cell::{Cell, RefCell};
use std::sync::{
    atomic::{AtomicBool, Ordering},
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

#[test]
fn app_default_does_not_follow_system_theme() {
    assert!(!App::default().follow_system_theme);
}

#[test]
fn app_builder_sets_follow_system_theme() {
    assert!(App::new().follow_system_theme(true).follow_system_theme);
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
