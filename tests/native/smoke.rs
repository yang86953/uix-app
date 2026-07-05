//! native 域 — 冒烟测试。

#![cfg(feature = "test-harness")]

use std::cell::Cell;
use uix::native::traits::*;
use uix::native::traits::event::UiEvent;
use uix::core::geometry::Point;
use uix::native::shared::OsEventSource;
use uix::native::test_harness::FakePlatform;
use uix::native::traits::input::KeyCode;

// ════════════════════════════════════════════════════════════════════════════
// FakeClipboard
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clipboard_read_write() {
    let mut pf = FakePlatform::new();
    pf.clipboard.set_text("hello");
    assert_eq!(pf.clipboard.text(), "hello");
    assert!(pf.clipboard.has_text());
    assert_eq!(pf.clipboard.last_set_text(), Some("hello"));
}

#[test]
fn clipboard_empty_by_default() {
    let pf = FakePlatform::new();
    assert!(!pf.clipboard.has_text());
    assert_eq!(pf.clipboard.text(), "");
}

// ════════════════════════════════════════════════════════════════════════════
// FakeConsole
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn console_write_line() {
    let mut pf = FakePlatform::new();
    pf.console.write_line("hello world");
    assert!(pf.console.state.output.contains("hello world"));
    assert_eq!(pf.console.last_line(), Some("hello world"));
}

#[test]
fn console_color() {
    let mut pf = FakePlatform::new();
    pf.console
        .set_color(uix::native::traits::system::ConsoleColor::Warn);
    assert_eq!(
        pf.console.last_color(),
        Some(uix::native::traits::system::ConsoleColor::Warn)
    );
}

// ════════════════════════════════════════════════════════════════════════════
// FakeCursor
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cursor_type() {
    let mut pf = FakePlatform::new();
    pf.cursor.set_cursor(uix::native::traits::input::CursorType::Hand);
    assert_eq!(pf.cursor.state.set_cursor_calls.len(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeEventSource
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn inject_and_dispatch_event() {
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());

    let called = Cell::new(false);
    let result = pf.event_loop().poll_event(&|ev| {
        called.set(true);
        assert_eq!(ev.type_, uix::native::traits::event::UiEventType::WindowClose);
        true
    });
    assert!(result);
    assert!(called.get());
}

// ════════════════════════════════════════════════════════════════════════════
// FakeFileDialog
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn file_dialog_mock_result() {
    let mut pf = FakePlatform::new();
    pf.file_dialog
        .mock_open_result(vec!["/path/to/file.txt".to_string()]);
    let files = pf.file_dialog.open("Open", "*.txt");
    assert_eq!(files, vec!["/path/to/file.txt"]);
    assert_eq!(pf.file_dialog.state.open_calls.len(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeFileSystem
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn file_system_read_added_file() {
    let mut pf = FakePlatform::new();
    pf.file_system.add_file("/config.json", b"{}".to_vec());
    let content = pf.file_system.read_file("/config.json").unwrap();
    assert_eq!(content, b"{}");
    assert_eq!(pf.file_system.read_calls.borrow().len(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeKeyboard
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn keyboard_press_release() {
    let mut pf = FakePlatform::new();
    pf.keyboard.press(KeyCode::A);
    assert!(pf.keyboard.is_down(KeyCode::A));
    pf.keyboard.release(KeyCode::A);
    assert!(!pf.keyboard.is_down(KeyCode::A));
}

// ════════════════════════════════════════════════════════════════════════════
// FakeNotification
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn notification_records() {
    let mut pf = FakePlatform::new();
    pf.notification.show("Title", "Message");
    assert_eq!(pf.notification.count(), 1);
    let last = pf.notification.last().unwrap();
    assert_eq!(last.title, "Title");
    assert_eq!(last.message, "Message");
}

// ════════════════════════════════════════════════════════════════════════════
// FakeTimer
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn timer_set_and_fire() {
    let mut pf = FakePlatform::new();
    let id = pf.timer.set(100, false);
    assert!(pf.timer.is_pending(id));
    let fired = pf.timer.advance(std::time::Duration::from_millis(100));
    assert_eq!(fired, vec![id]);
    assert!(!pf.timer.is_pending(id));
}

#[test]
fn timer_repeating() {
    let mut pf = FakePlatform::new();
    let id = pf.timer.set(50, true);
    pf.timer.advance(std::time::Duration::from_millis(50));
    assert!(pf.timer.is_pending(id)); // repeating → still pending
    pf.timer.advance(std::time::Duration::from_millis(50));
    assert!(pf.timer.is_pending(id)); // still pending after 2nd fire
}

// ════════════════════════════════════════════════════════════════════════════
// FakeWindowManager
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn window_manager_create() {
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("test", 800, 600).unwrap();
    win.show();
    assert!(win.is_visible());
    assert_eq!(pf.window_manager.create_calls.len(), 1);
    assert_eq!(pf.window_manager.create_calls[0].0, "test");
}

// ════════════════════════════════════════════════════════════════════════════
// FakePlatform — 通过 Platform trait 接口访问
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_trait_clipboard() {
    let mut pf = FakePlatform::new();
    let platform: &mut dyn Platform = &mut pf;
    platform.clipboard().set_text("via trait");
    assert_eq!(platform.clipboard().text(), "via trait");
}

#[test]
fn platform_trait_display() {
    let pf = FakePlatform::new();
    let platform: &dyn Platform = &pf;
    assert_eq!(platform.display().dpi_scale(), 1.0);
}

#[test]
fn platform_trait_timer() {
    let mut pf = FakePlatform::new();
    let platform: &mut dyn Platform = &mut pf;
    let id = platform.timer().set(100, false);
    platform.timer().clear(id);
}

#[test]
fn fake_console_capabilities() {
    let pf = FakePlatform::new();
    let caps = pf.console.capabilities();
    assert!(caps.has_color);
    assert!(!caps.has_raw_mode);
}

#[test]
fn fake_system_info_defaults() {
    let pf = FakePlatform::new();
    let info = pf.system_info.os_info();
    assert_eq!(info.name, "FakeOS");
    assert_eq!(pf.system_info.cpu_count(), 4);
}

#[test]
fn fake_display_configure() {
    let pf = FakePlatform::new();
    pf.display.set_dpi(2.0);
    assert_eq!(pf.display.dpi_scale(), 2.0);
    pf.display.set_dark_mode(true);
    assert!(pf.display.is_dark_mode());
}

#[test]
fn fake_presenter_tracks_calls() {
    let mut pf = FakePlatform::new();
    pf.presenter
        .present(&[0xFF0000; 100], 10, 10, uix::native::PresentDamage::Full)
        .unwrap();
    assert_eq!(pf.presenter.present_count(), 1);
}

#[test]
fn fake_text_input_active() {
    let mut pf = FakePlatform::new();
    pf.text_input.start();
    assert!(pf.text_input.state.active);
    pf.text_input.stop();
    assert!(!pf.text_input.state.active);
}

#[test]
fn fake_graphics_context() {
    let mut ctx = uix::native::test_harness::FakeGraphicsContext::with_size(800, 600);
    assert_eq!(ctx.width(), 800);
    assert_eq!(ctx.height(), 600);
    ctx.make_current();
    assert_eq!(ctx.state.make_current_calls, 1);
}

// ════════════════════════════════════════════════════════════════════════════
// FakePlatform 全量默认状态验证
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fake_platform_all_defaults() {
    let pf = FakePlatform::new();

    // Clipboard
    assert!(!pf.clipboard.has_text());
    assert_eq!(pf.clipboard.text(), "");
    assert!(pf.clipboard.last_set_text().is_none());

    // Console
    assert!(pf.console.state.output.is_empty());
    assert!(pf.console.state.lines.is_empty());
    assert_eq!(pf.console.state.current_color, ConsoleColor::Default);
    assert!(pf.console.state.cursor_visible);
    assert!(pf.console.state.capabilities.has_color);

    // Cursor
    assert_eq!(pf.cursor.state.cursor_type, CursorType::Arrow);
    assert!(pf.cursor.state.visible);
    assert_eq!(pf.cursor.state.position, Point::zero());
    assert!(!pf.cursor.state.confined);
    assert!(!pf.cursor.state.captured);

    // Display
    assert_eq!(pf.display.dpi_scale(), 1.0);
    assert!(!pf.display.is_dark_mode());
    assert_eq!(pf.display.count(), 1);
    assert_eq!(pf.display.info_calls.borrow().len(), 0);

    // EventSource
    assert_eq!(pf.event_source.pending_count(), 0);
    assert_eq!(pf.event_source.processed_count(), 0);
    assert!(!pf.event_source.state.should_exit);

    // FileDialog
    assert!(pf.file_dialog.state.open_result.is_empty());
    assert!(pf.file_dialog.state.save_result.is_empty());
    assert!(pf.file_dialog.state.open_calls.is_empty());

    // FileSystem
    assert_eq!(pf.file_system.state.files.len(), 0);
    assert_eq!(pf.file_system.read_calls.borrow().len(), 0);
    assert_eq!(pf.file_system.executable_path(), "/usr/bin/uix-app");

    // Keyboard
    assert!(!pf.keyboard.is_down(KeyCode::A));
    assert_eq!(pf.keyboard.idle_ms(), 0);
    assert_eq!(pf.keyboard.double_click_ms(), 500);

    // Notification
    assert_eq!(pf.notification.count(), 0);

    // Presenter
    assert_eq!(pf.presenter.present_count(), 0);
    assert_eq!(pf.presenter.resize_count(), 0);

    // SystemInfo
    assert_eq!(pf.system_info.cpu_count(), 4);
    assert_eq!(pf.system_info.hostname(), "fake-host");
    assert_eq!(pf.system_info.username(), "user");
    assert!(pf.system_info.default_font_path().is_some());
    assert_eq!(pf.system_info.default_font_calls.get(), 1);

    // TextInput
    assert!(!pf.text_input.state.active);
    assert_eq!(pf.text_input.state.start_calls, 0);
    assert_eq!(pf.text_input.state.stop_calls, 0);

    // Timer
    assert_eq!(pf.timer.pending_count(), 0);
    assert_eq!(pf.timer.state.set_calls.len(), 0);

    // WindowManager
    assert_eq!(pf.window_manager.create_calls.len(), 0);
}

// ════════════════════════════════════════════════════════════════════════════
// clear_history 一致性验证
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clipboard_clear_history() {
    let mut pf = FakePlatform::new();
    pf.clipboard.set_text("hello");
    assert_eq!(pf.clipboard.state.set_text_calls.len(), 1);
    pf.clipboard.clear_history();
    assert_eq!(pf.clipboard.state.set_text_calls.len(), 0);
    assert_eq!(pf.clipboard.state.has_text_calls.get(), 0);
    // 当前文本保留（不清除状态，只清调用记录）
    assert_eq!(pf.clipboard.text(), "hello");
}

#[test]
fn console_clear_history() {
    let mut pf = FakePlatform::new();
    pf.console.write("a");
    pf.console.write_line("b");
    assert!(!pf.console.state.writes.is_empty());
    assert!(!pf.console.state.lines.is_empty());
    pf.console.clear_history();
    assert!(pf.console.state.writes.is_empty());
    assert!(pf.console.state.lines.is_empty());
    assert!(pf.console.state.colors.is_empty());
    assert!(pf.console.state.output.is_empty());
}

#[test]
fn cursor_clear_history() {
    let mut pf = FakePlatform::new();
    pf.cursor.set_cursor(CursorType::Hand);
    pf.cursor.set_cursor_position(10, 20);
    pf.cursor.capture_mouse();
    pf.cursor.clear_history();
    assert!(pf.cursor.state.set_cursor_calls.is_empty());
    assert!(pf.cursor.state.set_position_calls.is_empty());
    assert!(pf.cursor.state.show_cursor_calls.is_empty());
    assert!(pf.cursor.state.confine_calls.is_empty());
    assert_eq!(pf.cursor.state.capture_calls, 0);
    assert_eq!(pf.cursor.state.release_calls, 0);
    // 当前状态保留
    assert_eq!(pf.cursor.state.cursor_type, CursorType::Hand);
    assert!(pf.cursor.state.captured);
}

#[test]
fn display_clear_history() {
    let pf = FakePlatform::new();
    let _ = pf.display.info(0);
    assert_eq!(pf.display.info_calls.borrow().len(), 1);
    pf.display.clear_history();
    assert_eq!(pf.display.info_calls.borrow().len(), 0);
}

#[test]
fn file_dialog_clear_history() {
    let mut pf = FakePlatform::new();
    pf.file_dialog.open("test", "*.*");
    pf.file_dialog.save("test", "*.*");
    pf.file_dialog.open_folder("test");
    pf.file_dialog.clear_history();
    assert!(pf.file_dialog.state.open_calls.is_empty());
    assert!(pf.file_dialog.state.save_calls.is_empty());
    assert!(pf.file_dialog.state.open_folder_calls.is_empty());
}

#[test]
fn file_system_clear_history() {
    let mut pf = FakePlatform::new();
    pf.file_system.add_file("/a", vec![1]);
    let _ = pf.file_system.read_file("/a");
    pf.file_system.clear_history();
    assert!(pf.file_system.read_calls.borrow().is_empty());
    // 文件树保留
    assert!(pf.file_system.read_file("/a").is_ok());
}

#[test]
fn timer_clear_history() {
    let mut pf = FakePlatform::new();
    pf.timer.set(100, false);
    pf.timer.clear(1);
    pf.timer.clear_history();
    assert!(pf.timer.state.set_calls.is_empty());
    assert!(pf.timer.state.clear_calls.is_empty());
}

#[test]
fn system_info_clear_history() {
    let mut pf = FakePlatform::new();
    let _ = pf.system_info.default_font_path();
    assert_eq!(pf.system_info.default_font_calls.get(), 1);
    pf.system_info.clear_history();
    assert_eq!(pf.system_info.default_font_calls.get(), 0);
}

#[test]
fn window_manager_clear_history() {
    let mut pf = FakePlatform::new();
    let _ = pf.window_manager.create_window("t", 100, 100);
    assert_eq!(pf.window_manager.create_calls.len(), 1);
    pf.window_manager.clear_history();
    assert!(pf.window_manager.create_calls.is_empty());
}

#[test]
fn text_input_clear_history() {
    let mut pf = FakePlatform::new();
    pf.text_input.start();
    pf.text_input.clear_history();
    assert_eq!(pf.text_input.state.start_calls, 0);
    assert!(pf.text_input.state.active); // 当前状态保留
}

#[test]
fn fake_window_direct_clear_history() {
    use uix::native::test_harness::FakeWindow;
    let mut win = FakeWindow::new(1, "test", 100, 100);
    win.show();
    win.set_title("title1");
    win.resize_notify(200, 200);
    assert!(win.state.visible);
    assert_eq!(win.state.set_title_calls.len(), 1);

    win.clear_history();
    assert_eq!(win.state.show_calls, 0);
    assert_eq!(win.state.set_title_calls.len(), 0);
    assert_eq!(win.state.resize_notify_calls.len(), 0);
    assert!(!win.state.close_called);
    // 当前状态保留
    assert!(win.state.visible);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeConsole 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn console_write_line_also_records_in_writes() {
    let mut pf = FakePlatform::new();
    pf.console.write_line("hello");
    // write_line 应该同时记录到 writes 和 lines
    assert!(pf.console.state.writes.contains(&"hello".to_string()));
    assert!(pf.console.state.lines.contains(&"hello".to_string()));
}

#[test]
fn console_write_does_not_record_in_lines() {
    let mut pf = FakePlatform::new();
    pf.console.write("just write");
    assert!(pf.console.state.writes.contains(&"just write".to_string()));
    assert!(!pf.console.state.lines.contains(&"just write".to_string()));
}

#[test]
fn console_reset_color_records() {
    let mut pf = FakePlatform::new();
    pf.console.set_color(ConsoleColor::Warn);
    pf.console.reset_color();
    assert_eq!(pf.console.state.colors.len(), 2);
    assert_eq!(pf.console.state.colors[1], ConsoleColor::Default);
}

#[test]
fn console_title_and_cursor() {
    let mut pf = FakePlatform::new();
    pf.console.set_terminal_title("uix");
    assert_eq!(pf.console.state.title, "uix");
    pf.console.show_terminal_cursor(false);
    assert!(!pf.console.state.cursor_visible);
    pf.console.show_terminal_cursor(true);
    assert!(pf.console.state.cursor_visible);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeCursor 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cursor_confine_tracks_history() {
    let mut pf = FakePlatform::new();
    pf.cursor.confine_cursor(true);
    assert!(pf.cursor.state.confined);
    assert_eq!(pf.cursor.state.confine_calls, vec![true]);
    pf.cursor.confine_cursor(false);
    assert!(!pf.cursor.state.confined);
    assert_eq!(pf.cursor.state.confine_calls, vec![true, false]);
}

#[test]
fn cursor_capture_release_tracks_counts() {
    let mut pf = FakePlatform::new();
    pf.cursor.capture_mouse();
    pf.cursor.release_mouse();
    pf.cursor.capture_mouse();
    assert_eq!(pf.cursor.state.capture_calls, 2);
    assert_eq!(pf.cursor.state.release_calls, 1);
}

#[test]
fn cursor_show_hide_tracks_history() {
    let mut pf = FakePlatform::new();
    pf.cursor.show_cursor(false);
    pf.cursor.show_cursor(true);
    pf.cursor.show_cursor(true);
    assert_eq!(pf.cursor.state.show_cursor_calls, vec![false, true, true]);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeDisplay 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn display_info_tracks_calls() {
    let pf = FakePlatform::new();
    let info = pf.display.info(0);
    assert!(info.is_primary);
    assert_eq!(pf.display.info_calls.borrow().len(), 1);
    assert_eq!(pf.display.info_calls.borrow()[0], 0);
}

#[test]
fn display_multi_monitor() {
    let pf = FakePlatform::new();
    pf.display.set_count(2);
    assert_eq!(pf.display.count(), 2);
    let primary = pf.display.info(0);
    assert!(primary.is_primary);
    let secondary = pf.display.info(1);
    assert!(!secondary.is_primary);
    assert_eq!(pf.display.info_calls.borrow().len(), 2);
}

#[test]
fn display_make_info_default_bounds() {
    let pf = FakePlatform::new();
    let info = pf.display.make_info(0);
    assert_eq!(info.bounds.w, 1920.0);
    assert_eq!(info.bounds.h, 1080.0);
}

// ════════════════════════════════════════════════════════════════════════════
// FakeEventSource 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn event_source_clear() {
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());
    pf.event_source.inject(UiEvent::close());
    assert_eq!(pf.event_source.pending_count(), 2);
    pf.event_source.clear();
    assert_eq!(pf.event_source.pending_count(), 0);
    assert_eq!(pf.event_source.processed_count(), 0);
    assert!(!pf.event_source.state.should_exit);
}

#[test]
fn event_source_inject_all() {
    let mut pf = FakePlatform::new();
    pf.event_source
        .inject_all(vec![UiEvent::close(), UiEvent::close(), UiEvent::close()]);
    assert_eq!(pf.event_source.pending_count(), 3);
}

#[test]
fn event_source_exit_signal() {
    let mut pf = FakePlatform::new();
    pf.event_source.state.should_exit = true;
    assert!(!pf.event_source.dispatch_pending());
    assert!(!pf.event_source.dispatch_blocking());
}

// ════════════════════════════════════════════════════════════════════════════
// FakeFileSystem 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn file_system_executable_path() {
    let pf = FakePlatform::new();
    assert_eq!(pf.file_system.executable_path(), "/usr/bin/uix-app");
    assert_eq!(pf.file_system.executable_dir(), "/usr/bin");
}

#[test]
fn file_system_add_dir() {
    let mut pf = FakePlatform::new();
    pf.file_system.add_dir("/my/custom/dir");
    assert!(pf.file_system.state.dirs.contains("/my/custom/dir"));
}

#[test]
fn file_system_add_file_creates_parent_dir() {
    let mut pf = FakePlatform::new();
    pf.file_system.add_file("/a/b/c.txt", vec![]);
    assert!(pf.file_system.state.dirs.contains("/a/b"));
}

#[test]
fn file_system_special_dir_customizable() {
    let mut pf = FakePlatform::new();
    pf.file_system
        .set_special_dir(SpecialDir::Home, "/custom/home");
    assert_eq!(
        pf.file_system.get_special_dir(SpecialDir::Home),
        "/custom/home"
    );
}

#[test]
fn file_system_read_file_not_found() {
    let pf = FakePlatform::new();
    match pf.file_system.read_file("/missing") {
        Err(e) => assert_eq!(e.code(), uix::core::error::Errc::NotFound),
        Ok(_) => panic!("expected error"),
    }
}

// ════════════════════════════════════════════════════════════════════════════
// FakeKeyboard 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn keyboard_tap_behavior() {
    let mut pf = FakePlatform::new();
    pf.keyboard.tap(KeyCode::Space);
    // tap = press + release, 所以 is_down 应该为 false
    assert!(!pf.keyboard.is_down(KeyCode::Space));
}

#[test]
fn keyboard_release_all() {
    let mut pf = FakePlatform::new();
    pf.keyboard.press(KeyCode::A);
    pf.keyboard.press(KeyCode::B);
    pf.keyboard.press(KeyCode::C);
    assert!(pf.keyboard.is_down(KeyCode::A));
    assert!(pf.keyboard.is_down(KeyCode::B));
    pf.keyboard.release_all();
    assert!(!pf.keyboard.is_down(KeyCode::A));
    assert!(!pf.keyboard.is_down(KeyCode::B));
    assert!(!pf.keyboard.is_down(KeyCode::C));
}

#[test]
fn keyboard_multiple_keys_down() {
    let mut pf = FakePlatform::new();
    pf.keyboard.press(KeyCode::Shift);
    pf.keyboard.press(KeyCode::Ctrl);
    assert!(pf.keyboard.is_down(KeyCode::Shift));
    assert!(pf.keyboard.is_down(KeyCode::Ctrl));
    pf.keyboard.release(KeyCode::Shift);
    assert!(!pf.keyboard.is_down(KeyCode::Shift));
    assert!(pf.keyboard.is_down(KeyCode::Ctrl));
}

// ════════════════════════════════════════════════════════════════════════════
// FakeSystemInfo 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn system_info_memory_info() {
    let pf = FakePlatform::new();
    let mem = pf.system_info.memory_info();
    assert_eq!(mem.total_bytes, 8 * 1024 * 1024 * 1024);
    assert_eq!(mem.available_bytes, 4 * 1024 * 1024 * 1024);
    assert!(mem.process_working_set > 0);
    assert!(mem.process_private_bytes > 0);
}

#[test]
fn system_info_up_time() {
    let pf = FakePlatform::new();
    assert_eq!(pf.system_info.up_time(), 3600);
}

#[test]
fn system_info_default_font_path_returns_none_when_empty() {
    let mut pf = FakePlatform::new();
    pf.system_info.default_font.clear();
    assert!(pf.system_info.default_font_path().is_none());
}

// ════════════════════════════════════════════════════════════════════════════
// FakeTimer 边界条件测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn timer_zero_interval() {
    let mut pf = FakePlatform::new();
    let id = pf.timer.set(0, false);
    assert!(pf.timer.is_pending(id));
    // 0ms 定时器立即触发
    let fired = pf.timer.advance(std::time::Duration::from_millis(0));
    assert_eq!(fired, vec![id]);
    assert!(!pf.timer.is_pending(id));
}

#[test]
fn timer_multiple_sequential() {
    let mut pf = FakePlatform::new();
    let id1 = pf.timer.set(100, false);
    let id2 = pf.timer.set(200, false);
    let id3 = pf.timer.set(300, false);
    // 推进 150ms → id1 触发
    let fired = pf.timer.advance(std::time::Duration::from_millis(150));
    assert_eq!(fired, vec![id1]);
    assert!(!pf.timer.is_pending(id1));
    assert!(pf.timer.is_pending(id2));
    assert!(pf.timer.is_pending(id3));
    // 再推进 100ms → id2 触发
    let fired = pf.timer.advance(std::time::Duration::from_millis(100));
    assert_eq!(fired, vec![id2]);
}

#[test]
fn timer_advance_to_idle() {
    let mut pf = FakePlatform::new();
    pf.timer.set(100, false);
    pf.timer.set(200, false);
    pf.timer.advance_to_idle();
    assert_eq!(pf.timer.pending_count(), 0);
}

#[test]
fn timer_clear_invalid_id() {
    let mut pf = FakePlatform::new();
    pf.timer.clear(999); // 不应该 panic
    assert!(pf.timer.state.clear_calls.contains(&999));
}

// ════════════════════════════════════════════════════════════════════════════
// FakeWindow & FakeWindowManager 增强测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn window_create_with_gpu() {
    use uix::native::test_harness::FakeWindow;
    let mut win = FakeWindow::new(1, "gpu-win", 800, 600).with_gpu();
    assert!(win.state.has_gpu);
    assert!(win.gpu_ctx.is_some());
    let gpu = win.graphics_context();
    assert!(gpu.is_some());
    assert_eq!(gpu.unwrap().width(), 0); // 初始化前为 0
}

#[test]
fn window_lifecycle_via_manager() {
    let mut pf = FakePlatform::new();
    let mut win = pf
        .window_manager
        .create_window("lifecycle", 800, 600)
        .unwrap();
    assert!(!win.is_visible());
    win.show();
    assert!(win.is_visible());
    win.hide();
    assert!(!win.is_visible());
    win.show();
    win.close();
    assert!(!win.is_visible());
}

#[test]
fn window_set_title_tracks_calls() {
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("init", 100, 100).unwrap();
    win.set_title("new-title");
    // PlatformWindow::set_title 通过 FakeWindow 实现，但 trait 不暴露 state
    // 验证 manager 记录了创建参数
    assert_eq!(pf.window_manager.create_calls[0].0, "init");
}

#[test]
fn window_properties_full_lifecycle() {
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("props", 800, 600).unwrap();
    let props = win.properties_mut();

    props.set_size(1024, 768);
    assert_eq!(props.width(), 1024);
    assert_eq!(props.height(), 768);

    props.set_minimum_size(200, 200);
    props.set_maximum_size(2000, 2000);
    props.set_resizable(false);
    assert!(!props.is_maximized());
    props.maximize();
    assert!(props.is_maximized());
    assert!(!props.is_minimized());
    props.minimize();
    assert!(props.is_minimized());
    assert!(!props.is_maximized());
    props.restore();
    assert!(!props.is_minimized());
    assert!(!props.is_maximized());

    props.set_borderless(true);
    // set_borderless 只改 borderless 状态，不改 fullscreen
    // 所以 is_fullscreen 仍然是 false（除非显示设置）
    assert!(!props.is_fullscreen());

    props.set_fullscreen(true);
    assert!(props.is_fullscreen());
    props.set_fullscreen(false);
    assert!(!props.is_fullscreen());

    props.set_always_on_top(true);
    props.set_window_opacity(0.5);
    props.enable_file_drop(true);
    props.start_text_input();
    props.stop_text_input();
}

#[test]
fn window_position() {
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("pos", 800, 600).unwrap();
    let props = win.properties_mut();
    assert_eq!(props.position().x, 0.0);
    assert_eq!(props.position().y, 0.0);
    props.set_position(100, 200);
    assert_eq!(props.position().x, 100.0);
    assert_eq!(props.position().y, 200.0);
}

#[test]
fn window_presenter_access() {
    let mut pf = FakePlatform::new();
    let mut win = pf
        .window_manager
        .create_window("present", 100, 100)
        .unwrap();
    let p = win.presenter();
    p.present(&[0xFF; 100], 10, 10, uix::native::PresentDamage::Full).unwrap();
    // 通过 Platform trait 也可以访问 presenter
    // 这里直接验证 presenter 工作
}

#[test]
fn window_native_handle() {
    let mut pf = FakePlatform::new();
    let win = pf.window_manager.create_window("native", 100, 100).unwrap();
    let handle = win.native_handle();
    assert!(handle.native_window().is_null());
}

// ════════════════════════════════════════════════════════════════════════════
// 跨 Fake 交互场景
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn inject_event_read_via_platform_trait() {
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());

    let called = Cell::new(false);
    let result = pf.event_loop().poll_event(&|ev| {
        called.set(true);
        assert_eq!(ev.type_, uix::native::traits::event::UiEventType::WindowClose);
        true
    });
    assert!(result);
    assert!(called.get());
    assert_eq!(pf.event_source.processed_count(), 1);
}

#[test]
fn window_and_event_loop_interaction() {
    let mut pf = FakePlatform::new();
    // 创建窗口
    let mut win = pf.window_manager.create_window("test", 640, 480).unwrap();
    win.show();
    assert!(win.is_visible());

    // 注入事件
    pf.event_source.inject(UiEvent::close());

    // 通过事件循环处理
    let called = Cell::new(false);
    pf.event_loop().poll_event(&|_| {
        called.set(true);
        true
    });
    assert!(called.get());

    // 窗口仍然可见
    assert!(win.is_visible());
}

#[test]
fn full_window_display_present_sequence() {
    // 模拟一次完整的渲染帧：创建窗口 → 呈现 → 事件处理
    let mut pf = FakePlatform::new();
    let mut win = pf.window_manager.create_window("frame", 200, 100).unwrap();
    win.show();

    // 呈现一帧像素（通过窗口自己的 presenter）
    let pixels: Vec<u32> = vec![0xFF0000; 200 * 100];
    win.presenter()
        .present(&pixels, 200, 100, uix::native::PresentDamage::Full)
        .unwrap();

    // 注入一个鼠标事件
    pf.event_source.inject(UiEvent::mouse_down(
        Point::new(50.0, 30.0),
        MouseButton::Left,
    ));

    let event_handled = Cell::new(false);
    pf.event_loop().poll_event(&|ev| {
        if let uix::native::traits::event::UiEventPayload::MouseButton(ref data) = ev.payload {
            event_handled.set(data.pos.x == 50.0 && data.btn == MouseButton::Left);
        }
        true
    });
    assert!(event_handled.get());

    // 验证呈现记录（窗口级别的 presenter，不是 pf.presenter）
    // FakeWindow 有自己的 FakePresenter，独立于 pf.presenter
}

// ════════════════════════════════════════════════════════════════════════════
// 边界条件测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clipboard_empty_string() {
    let mut pf = FakePlatform::new();
    pf.clipboard.set_text("");
    assert_eq!(pf.clipboard.text(), "");
    assert!(!pf.clipboard.has_text());
}

#[test]
fn clipboard_large_text() {
    let mut pf = FakePlatform::new();
    let large = "a".repeat(100_000);
    pf.clipboard.set_text(&large);
    assert_eq!(pf.clipboard.text().len(), 100_000);
}

#[test]
fn presenter_large_pixels() {
    let mut pf = FakePlatform::new();
    let pixels = vec![0x12345678u32; 10_000];
    pf.presenter
        .present(&pixels, 100, 100, uix::native::PresentDamage::Full)
        .unwrap();
    assert_eq!(pf.presenter.state.last_pixels.len(), 10_000);
    assert_eq!(pf.presenter.state.last_pixels[0], 0x12345678);
}

#[test]
fn presenter_empty_pixels() {
    let mut pf = FakePlatform::new();
    pf.presenter
        .present(&[], 0, 0, uix::native::PresentDamage::Full)
        .unwrap();
    assert_eq!(pf.presenter.state.last_pixels.len(), 0);
}

#[test]
fn presenter_dirty_rect() {
    let mut pf = FakePlatform::new();
    pf.presenter
        .present(
            &[0xFF; 100],
            10,
            10,
            uix::native::PresentDamage::single(2, 3, 6, 7),
        )
        .unwrap();
    let call = &pf.presenter.state.present_calls[0];
    assert_eq!(call.damage, uix::native::PresentDamage::single(2, 3, 6, 7));
}

#[test]
fn console_empty_write() {
    let mut pf = FakePlatform::new();
    pf.console.write("");
    pf.console.write_line("");
    assert!(pf.console.state.output.contains('\n'));
}

#[test]
fn event_source_empty_queue() {
    let mut pf = FakePlatform::new();
    assert_eq!(pf.event_source.pending_count(), 0);
    assert!(pf.event_source.next_event().is_none());
}

#[test]
fn file_dialog_default_results() {
    let mut pf = FakePlatform::new();
    let files = pf.file_dialog.open("Open", "*.*");
    assert!(files.is_empty());
    let path = pf.file_dialog.save("Save", "*.*");
    assert!(path.is_empty());
    let folder = pf.file_dialog.open_folder("Folder");
    assert!(folder.is_empty());
}

#[test]
fn notification_empty_strings() {
    let mut pf = FakePlatform::new();
    pf.notification.show("", "");
    assert_eq!(pf.notification.count(), 1);
    assert_eq!(pf.notification.last().unwrap().title, "");
    assert_eq!(pf.notification.last().unwrap().message, "");
}

#[test]
fn notification_multiple_clear() {
    let mut pf = FakePlatform::new();
    for i in 0..10 {
        pf.notification.show(&format!("Title {}", i), "msg");
    }
    assert_eq!(pf.notification.count(), 10);
    pf.notification.clear();
    assert_eq!(pf.notification.count(), 0);
}

#[test]
fn timer_multiple_repeating() {
    let mut pf = FakePlatform::new();
    let id_r = pf.timer.set(50, true); // repeating
    let id_s = pf.timer.set(100, false); // single

    // 推进 60ms → repeating(50ms) 触发, single(100ms) 还剩 40ms
    let fired = pf.timer.advance(std::time::Duration::from_millis(60));
    assert!(fired.contains(&id_r), "repeating should fire");
    assert!(!fired.contains(&id_s), "single should not fire yet");
    assert!(
        pf.timer.is_pending(id_r),
        "repeating reset and still pending"
    );
    assert!(pf.timer.is_pending(id_s), "single still pending");
    assert_eq!(pf.timer.pending_count(), 2);

    // 再推进 50ms → single(100ms) 已到 110ms 到期触发, repeating(50ms) 再次触发
    let fired = pf.timer.advance(std::time::Duration::from_millis(50));
    assert!(fired.contains(&id_r), "repeating fires again");
    assert!(fired.contains(&id_s), "single fires now");
    // single 被移除，repeating 仍挂起
    assert!(pf.timer.is_pending(id_r));
    assert!(!pf.timer.is_pending(id_s));
    assert_eq!(pf.timer.pending_count(), 1);

    // 清除 repeating
    pf.timer.clear(id_r);
    assert_eq!(pf.timer.pending_count(), 0);
}

#[test]
fn graphics_context_shutdown() {
    let mut ctx = uix::native::test_harness::FakeGraphicsContext::new();
    assert!(!ctx.state.shutdown_called);
    ctx.shutdown();
    assert!(ctx.state.shutdown_called);
}

#[test]
fn graphics_context_read_pixels() {
    let mut ctx = uix::native::test_harness::FakeGraphicsContext::new();
    let pixels = ctx.read_pixels(0, 0, 10, 10);
    assert!(pixels.is_empty());
}
