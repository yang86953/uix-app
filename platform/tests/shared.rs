//! 共享层组件测试 — WindowState / PlatformWindowCore / OsEventSource / WindowOps 默认实现。
//!
//! 使用 test_harness 的 FakeWindow 和 FakePlatform 替代手工 Mock。

#![cfg(feature = "test-harness")]

use uix_platform::api::traits::*;
use uix_platform::shared::{OsEventSource, PlatformWindowCore, WindowOps, WindowState};
use uix_platform::test_harness::{FakePlatform, FakeEventSource};
use uix_platform::event::UiEvent;
use uix_platform::presenter::NullPresenter;
use uix_platform::geometry::Point;
use uix_platform::types::{KeyCode, KeyMod};
use std::rc::Rc;
use std::cell::RefCell;

// ════════════════════════════════════════════════════════════════════════════
// WindowState — 纯状态机逻辑
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn window_state_default_values() {
    let state = WindowState::default();
    assert_eq!(state.width, 800);
    assert_eq!(state.height, 600);
    assert_eq!(state.pos_x, 0);
    assert_eq!(state.pos_y, 0);
    assert!(!state.visible);
    assert!(state.resizable);
    assert!(!state.borderless);
    assert!(!state.fullscreen);
    assert!(!state.maximized);
    assert!(!state.minimized);
    assert!(!state.always_on_top);
    assert_eq!(state.opacity, 1.0);
    assert!(!state.file_drop_enabled);
    assert!(!state.text_input_active);
}

#[test]
fn window_state_with_size() {
    let state = WindowState::with_size(1024, 768);
    assert_eq!(state.width, 1024);
    assert_eq!(state.height, 768);
}

#[test]
fn window_state_position() {
    let state = WindowState { pos_x: 50, pos_y: 100, ..WindowState::default() };
    let pos = state.position();
    assert_eq!(pos.x, 50.0);
    assert_eq!(pos.y, 100.0);
}

#[test]
fn window_state_clone() {
    let a = WindowState::with_size(640, 480);
    let b = a.clone();
    assert_eq!(a.width, b.width);
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps 默认实现验证
// ════════════════════════════════════════════════════════════════════════════

/// 最小 WindowOps 实现（只实现必须方法）
struct MinimalOps {
    handle: *mut std::ffi::c_void,
    pub show_called: bool,
    pub hide_called: bool,
    pub close_called: bool,
    pub last_title: Option<String>,
    pub last_size: Option<(i32, i32)>,
}

impl WindowOps for MinimalOps {
    fn os_show(&mut self) { self.show_called = true; }
    fn os_hide(&mut self) { self.hide_called = true; }
    fn os_close(&mut self) { self.close_called = true; }
    fn os_set_title(&mut self, title: &str) { self.last_title = Some(title.to_string()); }
    fn os_set_size(&mut self, w: i32, h: i32) { self.last_size = Some((w, h)); }
    fn native_handle(&self) -> *mut std::ffi::c_void { self.handle }
}

fn make_minimal_ops() -> MinimalOps {
    MinimalOps {
        handle: std::ptr::null_mut(),
        show_called: false,
        hide_called: false,
        close_called: false,
        last_title: None,
        last_size: None,
    }
}

#[test]
fn window_ops_default_impls_do_not_panic() {
    let mut ops = make_minimal_ops();
    ops.os_center_on_screen();
    ops.os_raise();
    ops.os_lower();
    ops.os_set_icon("icon.png");
    ops.os_flash();
    ops.os_set_min_size(100, 100);
    ops.os_set_max_size(500, 500);
    ops.os_set_position(10, 20);
    ops.os_set_resizable(true);
    ops.os_maximize();
    ops.os_minimize();
    ops.os_restore();
    ops.os_set_borderless(true);
    ops.os_set_fullscreen(false);
    ops.os_set_always_on_top(false);
    ops.os_set_opacity(0.5);
    ops.os_start_text_input();
    ops.os_stop_text_input();
    ops.os_enable_file_drop(true);
    ops.os_resize_notify(200, 300);
    // 所有默认实现应静默成功（打印 debug 日志）
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindowCore 组合验证
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn core_delegates_to_ops() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(Rc::clone(&state), ops, presenter);

    assert!(!core.is_visible());
    core.show();
    assert!(state.borrow().visible);
    core.hide();
    assert!(!state.borrow().visible);
    core.close();
    assert!(!state.borrow().visible);

    core.set_title("New Title");
    // title 存储在 ops 中，不在 WindowState 里

    core.resize_notify(1024, 768);
    assert_eq!(state.borrow().width, 1024);
    assert_eq!(state.borrow().height, 768);
}

#[test]
fn core_minimize_maximize_restore() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(Rc::clone(&state), ops, presenter);

    core.properties_mut().maximize();
    assert!(state.borrow().maximized);
    core.properties_mut().minimize();
    assert!(state.borrow().minimized);
    core.properties_mut().restore();
    assert!(!state.borrow().maximized);
    assert!(!state.borrow().minimized);
}

#[test]
fn core_properties_delegate() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(Rc::clone(&state), ops, presenter);

    assert_eq!(core.properties().width(), 800);
    core.properties_mut().set_size(1024, 768);
    assert_eq!(state.borrow().width, 1024);

    core.properties_mut().maximize();
    assert!(state.borrow().maximized);
    core.properties_mut().restore();
    assert!(!state.borrow().maximized);
}

#[test]
fn core_state_rc() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let core = PlatformWindowCore::new(Rc::clone(&state), ops, presenter);
    let rc = core.state_rc();
    assert!(Rc::ptr_eq(&rc, &state));
}

#[test]
fn core_presenter_access() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state, ops, presenter);
    let p = core.presenter();
    assert!(p.resize(100, 100).is_ok());
}

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource 行为验证
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn os_event_source_poll_dispatch_pending() {
    use std::cell::Cell;
    let mut source = FakeEventSource::new();
    source.inject(UiEvent::close());

    let called = Cell::new(false);
    let result = source.poll_event(&|_| { called.set(true); true });
    assert!(result);
    assert!(called.get());
}

#[test]
fn os_event_source_poll_returns_false_on_exit() {
    let mut source = FakeEventSource::new();
    source.state.should_exit = true;
    let result = source.poll_event(&|_| true);
    assert!(!result);
}

#[test]
fn os_event_source_poll_stops_on_false_callback() {
    use std::cell::Cell;
    let mut source = FakeEventSource::new();
    source.inject(UiEvent::close());
    source.inject(UiEvent::close());
    let count = Cell::new(0);
    let result = source.poll_event(&|_| {
        count.set(count.get() + 1);
        false
    });
    assert!(!result);
    assert_eq!(count.get(), 1);
}

#[test]
fn os_event_source_wait_event() {
    use std::cell::Cell;
    let mut source = FakeEventSource::new();
    source.inject(UiEvent::key_down(KeyCode::A, KeyMod::NONE));
    let called = Cell::new(false);
    let result = source.wait_event(&|_| { called.set(true); true });
    assert!(result);
    assert!(called.get());
}

#[test]
fn os_event_source_wait_timeout() {
    use std::cell::Cell;
    use std::time::Duration;
    let mut source = FakeEventSource::new();
    source.inject(UiEvent::close());
    let called = Cell::new(false);
    let result = source.wait_timeout(Duration::from_millis(10), &|_| { called.set(true); true });
    assert!(result);
    assert!(called.get());
}

#[test]
fn os_event_source_next_event_fifo() {
    let mut source = FakeEventSource::new();
    source.inject(UiEvent::close());
    source.inject(UiEvent::mouse_down(Point::new(0.0, 0.0), uix_platform::types::MouseButton::Left));
    assert_eq!(source.next_event().unwrap().type_, uix_platform::event::UiEventType::WindowClose);
    assert_eq!(
        source.next_event().unwrap().type_,
        uix_platform::event::UiEventType::MouseDown
    );
    assert!(source.next_event().is_none());
}

#[test]
fn os_event_source_processed_tracking() {
    let mut source = FakeEventSource::new();
    source.inject(UiEvent::close());
    let _ = source.next_event();
    assert_eq!(source.state.processed.len(), 1);
}

// ════════════════════════════════════════════════════════════════════════════
// 组合场景：FakePlatform 作为 OsEventSource
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn fake_platform_as_event_source() {
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());
    pf.event_source.inject(UiEvent::close());

    let count = std::cell::Cell::new(0);
    pf.event_loop().poll_event(&|_| {
        count.set(count.get() + 1);
        true
    });
    assert_eq!(count.get(), 2);
}

#[test]
fn fake_platform_poll_returns_false_on_exit() {
    let mut pf = FakePlatform::new();
    pf.event_source.state.should_exit = true;
    let result = pf.event_loop().poll_event(&|_| true);
    assert!(!result);
}
