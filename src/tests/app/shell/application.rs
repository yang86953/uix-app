use super::*;
use crate::app::app_timer::AppTimerQueue;
use crate::app::main_thread_queue::MainThreadQueue;
use crate::data::SettingsService;
use crate::native::test_harness::FakePlatform;
use crate::native::traits::event::{
    ClipboardData, ImeCompositionData, LocaleChangeData, ThemeChangeData,
};
use crate::ui::view::combinators::label;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};

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
    assert_eq!(secondary_windows[0]._session.window_id(), request.window_id);
    assert!(request.alive.load(Ordering::Acquire));
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
