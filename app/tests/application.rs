//! uix-app 应用层集成测试 —— Application / AppMode / map_ui_event

use uix_app::application::{App, AppMode, map_ui_event};
use uix_app::cli::{Cli, CliArgs};
use uix_platform::event::{UiEvent, UiEventType, UiEventPayload};
use uix_platform::geometry::Point;
use uix_platform::types::{KeyCode, KeyMod, MouseButton};
use uix_ui::widget::WidgetEvent;

// ════════════════════════════════════════════════════════════════════════════
// AppMode 枚举
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn app_mode_gui_variant() {
    assert_eq!(format!("{:?}", AppMode::GUI), "GUI");
}

#[test]
fn app_mode_cli_variant() {
    assert_eq!(format!("{:?}", AppMode::CLI), "CLI");
}

#[test]
fn app_mode_default_is_gui() {
    let mode: AppMode = Default::default();
    assert_eq!(mode, AppMode::GUI);
}

#[test]
fn app_mode_partial_eq() {
    assert_eq!(AppMode::GUI, AppMode::GUI);
    assert_eq!(AppMode::CLI, AppMode::CLI);
    assert_ne!(AppMode::GUI, AppMode::CLI);
}

// ════════════════════════════════════════════════════════════════════════════
// App builder 方法
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn new_creates_default_app() {
    let app = App::new();
    assert_eq!(app.current_mode(), AppMode::GUI);
    assert_eq!(app.window_title(), "UIX App");
    assert_eq!(app.window_size(), (800, 600));
    assert_eq!(app.exit_code(), 0);
}

#[test]
fn default_works_same_as_new() {
    let app_new = App::new();
    let app_default: App = Default::default();
    assert_eq!(app_new.window_title(), app_default.window_title());
    assert_eq!(app_new.window_size(), app_default.window_size());
    assert_eq!(app_new.current_mode(), app_default.current_mode());
    assert_eq!(app_new.exit_code(), app_default.exit_code());
}

#[test]
fn title_sets_window_title() {
    let mut app = App::new();
    app.title("自定义标题");
    assert_eq!(app.window_title(), "自定义标题");
}

#[test]
fn size_sets_window_size() {
    let mut app = App::new();
    app.size(1280, 720);
    assert_eq!(app.window_size(), (1280, 720));
}

#[test]
fn mode_sets_app_mode() {
    let mut app = App::new();
    assert_eq!(app.current_mode(), AppMode::GUI);
    app.mode(AppMode::CLI);
    assert_eq!(app.current_mode(), AppMode::CLI);
}

#[test]
fn cli_registers_cli() {
    fn handler(_: &CliArgs) -> i32 {
        0
    }
    let mut cli = Cli::new();
    cli.command("build", handler, "");
    let mut app = App::new();
    app.mode(AppMode::CLI).cli(cli);
    // CLI mode run reads std::env::args(); just verify no panic
    let code = app.run();
    assert!(code >= 0);
}

#[test]
fn container_returns_mutable_ref() {
    let mut app = App::new();
    let c = app.container();
    c.singleton(42i32);
    assert!(c.has::<i32>());
}

#[test]
fn singleton_registers_via_container() {
    let mut app = App::new();
    app.singleton("你好世界".to_string());
    assert!(app.container().has::<String>());
    if let Some(val) = app.container().resolve::<String>() {
        assert_eq!(val, "你好世界");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// App 查询方法
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn current_mode_returns_gui_by_default() {
    let app = App::new();
    assert_eq!(app.current_mode(), AppMode::GUI);
}

#[test]
fn window_title_returns_default() {
    let app = App::new();
    assert_eq!(app.window_title(), "UIX App");
}

#[test]
fn window_size_returns_default() {
    let app = App::new();
    assert_eq!(app.window_size(), (800, 600));
}

#[test]
fn exit_code_starts_at_zero() {
    let app = App::new();
    assert_eq!(app.exit_code(), 0);
}

#[test]
fn window_returns_none_before_create() {
    let app = App::new();
    assert!(app.window().is_none());
}

#[test]
fn window_mut_returns_none_before_create() {
    let mut app = App::new();
    assert!(app.window_mut().is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// App::run() CLI 路径
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn run_cli_mode_without_cli_returns_zero() {
    let mut app = App::new();
    app.mode(AppMode::CLI);
    let code = app.run();
    assert_eq!(code, 0);
}

#[test]
fn run_cli_mode_with_registered_cli_returns_exit_code() {
    fn handler(_: &CliArgs) -> i32 {
        42
    }
    let mut cli = Cli::new();
    cli.command("build", handler, "");
    let mut app = App::new();
    app.mode(AppMode::CLI).cli(cli);
    let code = app.run();
    // run() invokes cli.run(&std::env::args()); result depends on actual args
    assert!(code >= 0);
}

// ════════════════════════════════════════════════════════════════════════════
// map_ui_event — 正确载荷映射
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn map_mouse_down() {
    let ev = UiEvent::mouse_down(Point::new(10.0, 20.0), MouseButton::Left);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::MouseDown { pos, button, mods }) = result {
        assert_eq!(pos, Point::new(10.0, 20.0));
        assert_eq!(button, MouseButton::Left);
        assert_eq!(mods, KeyMod::NONE);
    }
}

#[test]
fn map_mouse_up() {
    let ev = UiEvent::mouse_up(Point::new(30.0, 40.0), MouseButton::Right);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::MouseUp { pos, button, mods }) = result {
        assert_eq!(pos, Point::new(30.0, 40.0));
        assert_eq!(button, MouseButton::Right);
        assert_eq!(mods, KeyMod::NONE);
    }
}

#[test]
fn map_mouse_move() {
    let ev = UiEvent::mouse_move(Point::new(50.0, 60.0));
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::MouseMove { pos }) = result {
        assert_eq!(pos, Point::new(50.0, 60.0));
    }
}

#[test]
fn map_mouse_wheel() {
    let ev = UiEvent::mouse_wheel(Point::new(70.0, 80.0), 2.0, -5.0, KeyMod::CTRL);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::MouseWheel { pos, delta }) = result {
        assert_eq!(pos, Point::new(70.0, 80.0));
        assert_eq!(delta, Point::new(2.0, -5.0));
    }
}

#[test]
fn map_key_down() {
    let ev = UiEvent::key_down(KeyCode::A, KeyMod::SHIFT);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::KeyDown { key, mods }) = result {
        assert_eq!(key, KeyCode::A);
        assert_eq!(mods, KeyMod::SHIFT);
    }
}

#[test]
fn map_key_up() {
    let ev = UiEvent::key_up(KeyCode::Escape, KeyMod::NONE);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::KeyUp { key, mods }) = result {
        assert_eq!(key, KeyCode::Escape);
        assert_eq!(mods, KeyMod::NONE);
    }
}

#[test]
fn map_key_press() {
    let ev = UiEvent::key_press("你好");
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::KeyPress { text }) = result {
        assert_eq!(text, "你好");
    }
}

#[test]
fn map_window_resize() {
    let ev = UiEvent::resize(1920, 1080);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::Resize { width, height }) = result {
        assert_eq!(width, 1920.0_f32);
        assert_eq!(height, 1080.0_f32);
    }
}

#[test]
fn map_window_maximize() {
    let ev = UiEvent {
        type_: UiEventType::WindowMaximize,
        payload: UiEventPayload::None,
    };
    let result = map_ui_event(&ev);
    assert!(matches!(result, Some(WidgetEvent::WindowMaximize)));
}

#[test]
fn map_window_minimize() {
    let ev = UiEvent {
        type_: UiEventType::WindowMinimize,
        payload: UiEventPayload::None,
    };
    let result = map_ui_event(&ev);
    assert!(matches!(result, Some(WidgetEvent::WindowMinimize)));
}

#[test]
fn map_window_restore() {
    let ev = UiEvent {
        type_: UiEventType::WindowRestore,
        payload: UiEventPayload::None,
    };
    let result = map_ui_event(&ev);
    assert!(matches!(result, Some(WidgetEvent::WindowRestore)));
}

#[test]
fn map_window_focus() {
    let ev = UiEvent {
        type_: UiEventType::WindowFocus,
        payload: UiEventPayload::None,
    };
    let result = map_ui_event(&ev);
    assert!(matches!(result, Some(WidgetEvent::WindowFocus)));
}

#[test]
fn map_window_blur() {
    let ev = UiEvent {
        type_: UiEventType::WindowBlur,
        payload: UiEventPayload::None,
    };
    let result = map_ui_event(&ev);
    assert!(matches!(result, Some(WidgetEvent::WindowBlur)));
}

#[test]
fn map_timer() {
    let ev = UiEvent::timer(7);
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::Timer { id }) = result {
        assert_eq!(id, 7);
    }
}

#[test]
fn map_file_drop() {
    let ev = UiEvent::file_drop(
        vec!["a.txt".to_string(), "b.png".to_string()],
        Point::new(100.0, 200.0),
    );
    let result = map_ui_event(&ev);
    assert!(result.is_some());
    if let Some(WidgetEvent::FileDrop { files, position }) = result {
        assert_eq!(files, vec!["a.txt", "b.png"]);
        assert_eq!(position, Point::new(100.0, 200.0));
    }
}

// ════════════════════════════════════════════════════════════════════════════
// map_ui_event — 边界情况
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn map_unknown_returns_none() {
    let ev = UiEvent {
        type_: UiEventType::Unknown,
        payload: UiEventPayload::None,
    };
    let result = map_ui_event(&ev);
    assert!(result.is_none());
}

#[test]
fn map_window_close_returns_none() {
    let ev = UiEvent::close();
    let result = map_ui_event(&ev);
    assert!(result.is_none());
}

#[test]
fn map_wrong_payload_type_returns_none() {
    // MouseDown expects MouseButton payload, but we provide None
    let ev = UiEvent::new(UiEventType::MouseDown, UiEventPayload::None);
    let result = map_ui_event(&ev);
    assert!(result.is_none());
}
