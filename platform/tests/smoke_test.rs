//! test_harness 冒烟测试 — 验证所有 Fake 子系统功能正确。

#![cfg(feature = "test-harness")]

use uix_platform::test_harness::FakePlatform;
use uix_platform::api::traits::*;
use std::cell::Cell;
use uix_platform::event::UiEvent;
use uix_platform::types::KeyCode;

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
    pf.console.set_color(uix_platform::types::ConsoleColor::Warn);
    assert_eq!(pf.console.last_color(), Some(uix_platform::types::ConsoleColor::Warn));
}

// ════════════════════════════════════════════════════════════════════════════
// FakeCursor
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn cursor_type() {
    let mut pf = FakePlatform::new();
    pf.cursor.set_cursor(uix_platform::types::CursorType::Hand);
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
        assert_eq!(ev.type_, uix_platform::event::UiEventType::WindowClose);
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
    pf.file_dialog.mock_open_result(vec!["/path/to/file.txt".to_string()]);
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
    pf.presenter.present(&[0xFF0000; 100], 10, 10, None).unwrap();
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
    let mut ctx = uix_platform::test_harness::FakeGraphicsContext::with_size(800, 600);
    assert_eq!(ctx.width(), 800);
    assert_eq!(ctx.height(), 600);
    ctx.make_current();
    assert_eq!(ctx.state.make_current_calls, 1);
}
