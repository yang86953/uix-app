//! native 域 — 平台契约集成测试。
//!
//! 每个测试验证：
//!   1. 可通过 `Platform` trait 访问子系统
//!   2. 写入后读出值一致
//!   3. Fake 的状态追踪正常工作

#![cfg(feature = "test-harness")]

use std::rc::Rc;
use uix::native::traits::*;
use uix::native::traits::event::UiEvent;
use uix::core::geometry::{EdgeInsets, Point, Rect, Size};
use uix::native::test_harness::FakePlatform;

// ════════════════════════════════════════════════════════════════════════════
// Platform trait — 所有访问器方法可用
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_trait_all_accessors_available() {
    let mut pf = FakePlatform::new();
    let p: &mut dyn Platform = &mut pf;
    let _ = p.window_manager();
    let _ = p.event_loop();
    let _ = p.event_bus();
    let _ = p.clipboard();
    let _ = p.cursor();
    let _ = p.file_dialog();
    let _ = p.text_input();
    let _ = p.timer();
    let _ = p.notification();
    let _ = p.console();
    let _ = p.display();
    let _ = p.keyboard();
    let _ = p.file_system();
    let _ = p.system_info();
}

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 像素呈现
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn presenter_default_empty() {
    let pf = FakePlatform::new();
    assert_eq!(pf.presenter.present_count(), 0);
}

#[test]
fn presenter_present_stores_pixels() {
    let mut pf = FakePlatform::new();
    pf.presenter
        .present(&[0xFF0000, 0x00FF00], 2, 1, uix::native::PresentDamage::Full)
        .unwrap();
    assert_eq!(pf.presenter.present_count(), 1);
    assert_eq!(pf.presenter.state.last_pixels.len(), 2);
}

#[test]
fn presenter_resize_tracks_calls() {
    let mut pf = FakePlatform::new();
    pf.presenter.resize(1920, 1080).unwrap();
    assert_eq!(pf.presenter.resize_count(), 1);
    assert_eq!(pf.presenter.state.resize_calls[0], (1920, 1080));
}

// ════════════════════════════════════════════════════════════════════════════
// IClipboard — 剪贴板
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clipboard_read_write_roundtrip() {
    let mut pf = FakePlatform::new();
    pf.clipboard.set_text("uix-framework");
    assert_eq!(pf.clipboard.text(), "uix-framework");
    assert!(pf.clipboard.has_text());
    assert_eq!(pf.clipboard.last_set_text(), Some("uix-framework"));
}

#[test]
fn clipboard_overwrite() {
    let mut pf = FakePlatform::new();
    pf.clipboard.set_text("first");
    pf.clipboard.set_text("second");
    assert_eq!(pf.clipboard.text(), "second");
    assert_eq!(pf.clipboard.state.set_text_calls.len(), 2);
}

#[test]
fn clipboard_empty_initially() {
    let pf = FakePlatform::new();
    assert!(!pf.clipboard.has_text());
    assert_eq!(pf.clipboard.text(), "");
}

// ════════════════════════════════════════════════════════════════════════════
// ICursor — 光标
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cursor_set_type() {
    let mut pf = FakePlatform::new();
    pf.cursor.set_cursor(CursorType::Hand);
    assert_eq!(pf.cursor.state.cursor_type, CursorType::Hand);
}

#[test]
fn cursor_position() {
    let mut pf = FakePlatform::new();
    pf.cursor.set_cursor_position(100, 200);
    let pos = pf.cursor.cursor_position();
    assert_eq!(pos.x, 100.0);
    assert_eq!(pos.y, 200.0);
}

#[test]
fn cursor_show_hide() {
    let mut pf = FakePlatform::new();
    pf.cursor.show_cursor(false);
    assert!(!pf.cursor.state.visible);
    pf.cursor.show_cursor(true);
    assert!(pf.cursor.state.visible);
}

#[test]
fn cursor_capture_release() {
    let mut pf = FakePlatform::new();
    pf.cursor.capture_mouse();
    assert!(pf.cursor.state.captured);
    pf.cursor.release_mouse();
    assert!(!pf.cursor.state.captured);
}

// ════════════════════════════════════════════════════════════════════════════
// IKeyboard — 键盘
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn keyboard_press_detects_down() {
    let mut pf = FakePlatform::new();
    assert!(!pf.keyboard.is_down(KeyCode::Space));
    pf.keyboard.press(KeyCode::Space);
    assert!(pf.keyboard.is_down(KeyCode::Space));
    pf.keyboard.release(KeyCode::Space);
    assert!(!pf.keyboard.is_down(KeyCode::Space));
}

#[test]
fn keyboard_idle_ms_default() {
    let pf = FakePlatform::new();
    assert_eq!(pf.keyboard.idle_ms(), 0);
}

#[test]
fn keyboard_double_click_ms_default() {
    let pf = FakePlatform::new();
    assert_eq!(pf.keyboard.double_click_ms(), 500);
}

// ════════════════════════════════════════════════════════════════════════════
// IDisplay — 显示
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn display_defaults() {
    let pf = FakePlatform::new();
    assert_eq!(pf.display.dpi_scale(), 1.0);
    assert!(!pf.display.is_dark_mode());
    assert_eq!(pf.display.count(), 1);
}

#[test]
fn display_configure() {
    let pf = FakePlatform::new();
    pf.display.set_dpi(2.0);
    pf.display.set_dark_mode(true);
    assert_eq!(pf.display.dpi_scale(), 2.0);
    assert!(pf.display.is_dark_mode());
}

// ════════════════════════════════════════════════════════════════════════════
// IConsole — 控制台
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn console_write_appends() {
    let mut pf = FakePlatform::new();
    pf.console.write("hello ");
    pf.console.write("world");
    assert!(pf.console.state.output.contains("hello world"));
}

#[test]
fn console_write_line_stores_lines() {
    let mut pf = FakePlatform::new();
    pf.console.write_line("line1");
    pf.console.write_line("line2");
    assert_eq!(pf.console.state.lines.len(), 2);
    assert_eq!(pf.console.last_line(), Some("line2"));
}

#[test]
fn console_color_tracking() {
    let mut pf = FakePlatform::new();
    pf.console.set_color(ConsoleColor::Warn);
    assert_eq!(pf.console.state.current_color, ConsoleColor::Warn);
    pf.console.reset_color();
    assert_eq!(pf.console.state.current_color, ConsoleColor::Default);
}

#[test]
fn console_title() {
    let mut pf = FakePlatform::new();
    pf.console.set_terminal_title("uix-test");
    assert_eq!(pf.console.state.title, "uix-test");
}

// ════════════════════════════════════════════════════════════════════════════
// IFileDialog — 文件对话框
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn file_dialog_open_mock() {
    let mut pf = FakePlatform::new();
    pf.file_dialog
        .mock_open_result(vec!["/path/a.txt".into(), "/path/b.txt".into()]);
    let files = pf.file_dialog.open("Open", "*.txt");
    assert_eq!(files, vec!["/path/a.txt", "/path/b.txt"]);
    assert_eq!(pf.file_dialog.state.open_calls[0].0, "Open");
}

#[test]
fn file_dialog_save_mock() {
    let mut pf = FakePlatform::new();
    pf.file_dialog.mock_save_result("/out/result.txt");
    let path = pf.file_dialog.save("Save", "*.txt");
    assert_eq!(path, "/out/result.txt");
}

// ════════════════════════════════════════════════════════════════════════════
// IFileSystem — 文件系统
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn file_system_add_and_read() {
    let mut pf = FakePlatform::new();
    pf.file_system.add_file("/hello.txt", b"world".to_vec());
    let content = pf.file_system.read_file("/hello.txt").unwrap();
    assert_eq!(content, b"world");
}

#[test]
fn file_system_read_missing_returns_error() {
    let pf = FakePlatform::new();
    let result = pf.file_system.read_file("/nonexistent");
    assert!(result.is_err());
}

#[test]
fn file_system_tracks_reads() {
    let mut pf = FakePlatform::new();
    pf.file_system.add_file("/a", vec![1]);
    let _ = pf.file_system.read_file("/a");
    let _ = pf.file_system.read_file("/a");
    assert_eq!(pf.file_system.read_calls.borrow().len(), 2);
}

#[test]
fn file_system_special_dirs() {
    let pf = FakePlatform::new();
    assert_eq!(
        pf.file_system.get_special_dir(SpecialDir::Home),
        "/home/user"
    );
    assert_eq!(pf.file_system.get_special_dir(SpecialDir::Temp), "/tmp");
}

// ════════════════════════════════════════════════════════════════════════════
// INotification — 通知
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn notification_shows_and_records() {
    let mut pf = FakePlatform::new();
    pf.notification.show("Test", "Hello");
    assert_eq!(pf.notification.count(), 1);
    let r = pf.notification.last().unwrap();
    assert_eq!(r.title, "Test");
    assert_eq!(r.message, "Hello");
}

#[test]
fn notification_multiple() {
    let mut pf = FakePlatform::new();
    pf.notification.show("A", "1");
    pf.notification.show("B", "2");
    assert_eq!(pf.notification.count(), 2);
    assert_eq!(pf.notification.find_by_title("A").len(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// ITimer — 定时器
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn timer_set_and_clear() {
    let mut pf = FakePlatform::new();
    let id = pf.timer.set(1000, false);
    assert!(pf.timer.is_pending(id));
    pf.timer.clear(id);
    assert!(!pf.timer.is_pending(id));
}

#[test]
fn timer_single_shot_fires_once() {
    let mut pf = FakePlatform::new();
    let id = pf.timer.set(100, false);
    let fired = pf.timer.advance(std::time::Duration::from_millis(100));
    assert_eq!(fired, vec![id]);
    assert!(!pf.timer.is_pending(id));
}

#[test]
fn timer_repeating_fires_multiple() {
    let mut pf = FakePlatform::new();
    pf.timer.set(50, true);
    pf.timer.advance(std::time::Duration::from_millis(50));
    assert_eq!(pf.timer.pending_count(), 1);
    pf.timer.advance(std::time::Duration::from_millis(50));
    assert_eq!(pf.timer.pending_count(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// ITextInput — 文字输入
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn text_input_start_stop() {
    let mut pf = FakePlatform::new();
    assert!(!pf.text_input.state.active);
    pf.text_input.start();
    assert!(pf.text_input.state.active);
    pf.text_input.stop();
    assert!(!pf.text_input.state.active);
}

// ════════════════════════════════════════════════════════════════════════════
// ISystemInfo — 系统信息
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn system_info_defaults() {
    let pf = FakePlatform::new();
    let os = pf.system_info.os_info();
    assert_eq!(os.name, "FakeOS");
    assert_eq!(pf.system_info.cpu_count(), 4);
    assert!(pf.system_info.memory_info().total_bytes > 0);
    assert_eq!(pf.system_info.hostname(), "fake-host");
    assert_eq!(pf.system_info.username(), "user");
}

// ════════════════════════════════════════════════════════════════════════════
// 事件分发 — IEventLoop
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn event_loop_polls_injected_events() {
    use std::cell::Cell;
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());
    let called = Cell::new(false);
    pf.event_loop().poll_event(&|ev| {
        assert_eq!(ev.type_, uix::native::traits::event::UiEventType::WindowClose);
        called.set(true);
        true
    });
    assert!(called.get());
}

#[test]
fn event_loop_multiple_events() {
    use std::cell::Cell;
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());
    pf.event_source.inject(UiEvent::close());
    let count = Cell::new(0);
    pf.event_loop().poll_event(&|_| {
        count.set(count.get() + 1);
        true
    });
    assert_eq!(count.get(), 2);
}

#[test]
fn event_loop_stops_on_false() {
    use std::cell::Cell;
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());
    pf.event_source.inject(UiEvent::close());
    let count = Cell::new(0);
    let result = pf.event_loop().poll_event(&|_| {
        let prev = count.get();
        count.set(prev + 1);
        prev < 0 // first call → false → stops
    });
    assert!(!result);
    assert_eq!(count.get(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// IWindowManager + PlatformWindow
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn window_create_and_show() {
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("test", 640, 480).unwrap();
    assert!(!win.is_visible());
    win.show();
    assert!(win.is_visible());
    assert_eq!(pf.window_manager.create_calls.len(), 1);
}

#[test]
fn window_properties() {
    let mut pf = FakePlatform::new();
    let mut win = pf
        .window_manager
        .create_window("prop-test", 800, 600)
        .unwrap();
    assert_eq!(win.properties().width(), 800);
    win.properties_mut().set_size(1024, 768);
    assert_eq!(win.properties().width(), 1024);
}

#[test]
fn window_title() {
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("", 100, 100).unwrap();
    win.set_title("My Window");
    // 无法读取 boxed PlatformWindow 的 title——但可以通过 manager 的 create_calls 验证
    assert_eq!(pf.window_manager.create_calls[0].0, "");
}

#[test]
fn window_close() {
    let mut pf = FakePlatform::new();
    let mut win = pf
        .window_manager
        .create_window("close-test", 100, 100)
        .unwrap();
    win.show();
    assert!(win.is_visible());
    win.close();
    assert!(!win.is_visible());
}

// ════════════════════════════════════════════════════════════════════════════
// IGraphicsContext
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn graphics_context_init() {
    let mut ctx = uix::native::test_harness::FakeGraphicsContext::new();
    ctx.initialize(std::ptr::null_mut(), 800, 600).unwrap();
    assert_eq!(ctx.width(), 800);
    assert_eq!(ctx.height(), 600);
}

#[test]
fn graphics_context_swap() {
    let mut ctx = uix::native::test_harness::FakeGraphicsContext::new();
    ctx.make_current();
    ctx.swap_buffers();
    assert_eq!(ctx.state.make_current_calls, 1);
    assert_eq!(ctx.state.swap_buffers_calls, 1);
}

// ════════════════════════════════════════════════════════════════════════════
// 类型可见性 — 确保公开类型可通过 uix_platform 访问
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn types_available() {
    let _p = Point::new(1.0, 2.0);
    let _s = Size::new(100.0, 200.0);
    let _r = Rect::new(0.0, 0.0, 100.0, 200.0);
    let _e = EdgeInsets::new(1.0, 2.0, 3.0, 4.0);
    let _err = uix::core::error::Error::new(uix::core::error::Errc::None, "");
    let _ = StatusLevel::Info;
    let _ = ConsoleColor::Default;
    let _ = CursorType::Arrow;
    let _ = KeyCode::A;
    let _ = KeyMod::NONE;
    let _ = ScrollDirection::Both;
    let _ = ControlSize::Medium;
    let _ = SpecialDir::Home;
    let _bus = uix::native::traits::event::EventBus::new();
    let _ev = UiEvent::close();
    let _ = uix::native::traits::event::UiEventType::WindowClose;
}

#[test]
fn prelude_via_star_import() {
    use uix::native::traits::*;
    let _p = Point::new(0.0, 0.0);
    let _err: uix::core::error::Error =
        uix::core::error::Error::new(uix::core::error::Errc::None, "");
    let _ev = UiEvent::close();
    let _bus = uix::native::traits::event::EventBus::new();
    let _kc = KeyCode::Enter;
    // traits available
    let _: &dyn IPresenter = &uix::native::presenter::NullPresenter;
}

// ════════════════════════════════════════════════════════════════════════════
// Platform trait 多子系统交互
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_mut_and_immutable_accessors() {
    // 验证可变和不可变访问器可以同时工作
    let mut pf = FakePlatform::new();
    let p: &mut dyn Platform = &mut pf;

    // 通过可变引用写入
    p.clipboard().set_text("hello");

    // 通过不可变引用读取（display / keyboard / file_system / system_info）
    let _dpi = p.display().dpi_scale();
    let _kc = p.keyboard().double_click_ms();
    let _home = p.file_system().get_special_dir(SpecialDir::Home);
    let _cpu = p.system_info().cpu_count();

    // 验证写入的内容
    assert_eq!(p.clipboard().text(), "hello");
}

#[test]
fn platform_event_bus_integration() {
    // 通过 FakePlatform.event_bus 直接访问
    let mut pf = FakePlatform::new();

    let called = Rc::new(std::cell::Cell::new(false));
    let c = called.clone();
    pf.event_bus
        .subscribe(uix::native::traits::event::UiEventType::MouseDown, move |_| {
            c.set(true);
            true
        });

    pf.event_bus.publish(&UiEvent::mouse_down(
        Point::new(10.0, 10.0),
        MouseButton::Left,
    ));

    assert!(called.get(), "direct event_bus should work");
}

#[test]
fn platform_event_bus_via_trait() {
    let mut pf = FakePlatform::new();
    let called = Rc::new(std::cell::Cell::new(false));
    {
        let c = called.clone();
        let p: &mut dyn Platform = &mut pf;
        p.event_bus()
            .subscribe(uix::native::traits::event::UiEventType::MouseDown, move |_| {
                c.set(true);
                true
            });
    }
    // 通过具体引用来 publish
    pf.event_bus.publish(&UiEvent::mouse_down(
        Point::new(10.0, 10.0),
        MouseButton::Left,
    ));

    assert!(called.get(), "subscribe via trait, publish via concrete");
}

#[test]
fn platform_clipboard_via_trait_has_text() {
    let mut pf = FakePlatform::new();
    let p: &mut dyn Platform = &mut pf;
    assert!(!p.clipboard().has_text());
    p.clipboard().set_text("data");
    assert!(p.clipboard().has_text());
}

#[test]
fn platform_cursor_full_lifecycle() {
    let mut pf = FakePlatform::new();
    {
        let p: &mut dyn Platform = &mut pf;
        p.cursor().set_cursor(CursorType::Hand);
        p.cursor().set_cursor_position(500, 300);
        p.cursor().confine_cursor(true);
        p.cursor().capture_mouse();
        p.cursor().show_cursor(false);
    }
    // 通过直接字段验证
    assert!(!pf.cursor.state.visible);
    assert_eq!(pf.cursor.state.cursor_type, CursorType::Hand);
}

#[test]
fn platform_file_system_and_dialog_interaction() {
    // 模拟一个常见场景：文件对话框选择一个文件，然后读取它
    let mut pf = FakePlatform::new();

    // 预设文件系统中有文件
    pf.file_system
        .add_file("/data/config.json", b"{\"key\": \"value\"}".to_vec());

    // 预设文件对话框返回该路径
    pf.file_dialog
        .mock_open_result(vec!["/data/config.json".to_string()]);

    // 测试流程：打开对话框 → 读取文件
    let files = pf.file_dialog.open("Open Config", "*.json");
    assert_eq!(files.len(), 1);

    let content = pf.file_system.read_file(&files[0]).unwrap();
    assert_eq!(content, b"{\"key\": \"value\"}");

    assert_eq!(pf.file_dialog.state.open_calls.len(), 1);
    assert_eq!(pf.file_system.read_calls.borrow().len(), 1);
}

#[test]
fn platform_console_and_notification() {
    let mut pf = FakePlatform::new();
    let p: &mut dyn Platform = &mut pf;

    p.console().write_line("Application started");
    p.notification().show("Info", "Ready");

    // 通过 Fake 直接断言
    assert!(pf.console.state.output.contains("Application started"));
    assert_eq!(pf.notification.count(), 1);
}

#[test]
fn platform_timer_multiple_shots() {
    let mut pf = FakePlatform::new();
    let p: &mut dyn Platform = &mut pf;

    let id1 = p.timer().set(100, false);
    let id2 = p.timer().set(200, true);

    p.timer().clear(id1);
    assert!(!pf.timer.is_pending(id1));
    assert!(pf.timer.is_pending(id2));
}

#[test]
fn platform_text_input_via_trait() {
    let mut pf = FakePlatform::new();

    {
        let p: &mut dyn Platform = &mut pf;
        p.text_input().start();
    }
    assert!(pf.text_input.state.active);
    {
        let p: &mut dyn Platform = &mut pf;
        p.text_input().stop();
    }
    assert!(!pf.text_input.state.active);
}

#[test]
fn platform_system_info_default_font() {
    let pf = FakePlatform::new();
    let p: &dyn Platform = &pf;
    let font = p.system_info().default_font_path();
    assert!(font.is_some());
}

#[test]
fn platform_window_creation_through_trait() {
    let mut pf = FakePlatform::new();
    let p: &mut dyn Platform = &mut pf;
    let mut win = p
        .window_manager()
        .create_window("trait-win", 640, 480)
        .unwrap();
    win.show();
    assert!(win.is_visible());
    assert_eq!(pf.window_manager.create_calls.len(), 1);
    assert_eq!(pf.window_manager.create_calls[0].0, "trait-win");
}

#[test]
fn platform_event_loop_multiple_types() {
    use std::cell::Cell;
    let mut pf = FakePlatform::new();

    pf.event_source.inject(UiEvent::close());
    pf.event_source
        .inject(UiEvent::key_down(KeyCode::Escape, KeyMod::NONE));
    pf.event_source
        .inject(UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));

    let types = Cell::new(Vec::new());
    pf.event_loop().poll_event(&|ev| {
        let mut t = types.take();
        t.push(ev.type_);
        types.set(t);
        true
    });

    let recorded = types.take();
    assert_eq!(recorded.len(), 3);
    assert_eq!(recorded[0], uix::native::traits::event::UiEventType::WindowClose);
    assert_eq!(recorded[1], uix::native::traits::event::UiEventType::KeyDown);
    assert_eq!(recorded[2], uix::native::traits::event::UiEventType::MouseDown);
}

#[test]
fn platform_display_info_via_trait() {
    let pf = FakePlatform::new();
    let p: &dyn Platform = &pf;

    let info = p.display().info(0);
    assert!(info.is_primary);
    assert_eq!(info.dpi_scale, 1.0);
}

#[test]
fn platform_fake_platform_ref_equality() {
    // 验证通过 Platform trait 和直接字段访问操作的是同一个状态
    let mut pf = FakePlatform::new();
    {
        let p: &mut dyn Platform = &mut pf;
        p.clipboard().set_text("shared");
    }
    // 直接读取应该看到同样的值
    assert_eq!(pf.clipboard.text(), "shared");
}
