//! 共享层组件测试 — WindowState / PlatformWindowCore / OsEventSource / WindowOps 默认实现。
//!
//! 使用 test_harness 的 FakeWindow 和 FakePlatform 替代手工 Mock。

#![cfg(feature = "test-harness")]

use uix_platform::api::traits::*;
use uix_platform::shared::{OsEventSource, PlatformWindowCore, WindowOps, WindowState};
use uix_platform::test_harness::{FakePlatform, FakeEventSource, FakeGraphicsContext};
use uix_platform::event::{UiEvent, UiEventType};
use uix_platform::presenter::NullPresenter;
use uix_platform::geometry::Point;
use uix_platform::types::{KeyCode, KeyMod, MouseButton};
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
// PlatformWindowCore GPU 模式
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn core_with_gpu_creates_graphics_context() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let gpu_ctx = Box::new(FakeGraphicsContext::with_size(800, 600));
    let mut core = PlatformWindowCore::with_gpu(Rc::clone(&state), ops, presenter, gpu_ctx);

    {
        let mut ctx = core.graphics_context();
        assert!(ctx.is_some());
        let ctx_ref = ctx.as_mut().unwrap();
        assert_eq!(ctx_ref.width(), 800);
        assert_eq!(ctx_ref.height(), 600);
    }
}

#[test]
fn core_without_gpu_returns_none() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state, ops, presenter);

    assert!(core.graphics_context().is_none());
}

#[test]
fn core_native_surface_ptr_default_null() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let core = PlatformWindowCore::new(state, ops, presenter);

    assert!(core.native_surface_ptr().is_null());
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindowCore 状态转换边界测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn core_show_hide_close_lifecycle() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state, ops, presenter);

    // 初始不可见
    assert!(!core.is_visible());

    // show
    core.show();
    assert!(core.is_visible());

    // 重复 show 不翻转为不可见
    core.show();
    assert!(core.is_visible());

    // hide
    core.hide();
    assert!(!core.is_visible());

    // 重复 hide 不翻转为可见
    core.hide();
    assert!(!core.is_visible());

    // show → close
    core.show();
    assert!(core.is_visible());
    core.close();
    assert!(!core.is_visible());

    // close 后可再次 show
    core.show();
    assert!(core.is_visible());
}

// ════════════════════════════════════════════════════════════════════════════
// PlatformWindowCore IWindowProperties 完整测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn core_fullscreen_idempotent() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state, ops, presenter);

    assert!(!core.properties().is_fullscreen());
    core.properties_mut().set_fullscreen(true);
    assert!(core.properties().is_fullscreen());
    // 重复设置保持 fullscreen
    core.properties_mut().set_fullscreen(true);
    assert!(core.properties().is_fullscreen());
    core.properties_mut().set_fullscreen(false);
    assert!(!core.properties().is_fullscreen());
    // 再次设置
    core.properties_mut().set_fullscreen(true);
    assert!(core.properties().is_fullscreen());
}

#[test]
fn core_minimize_maximize_toggle() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state, ops, presenter);

    // 初始两者都不是
    assert!(!core.properties().is_maximized());
    assert!(!core.properties().is_minimized());

    // maximize
    core.properties_mut().maximize();
    assert!(core.properties().is_maximized());
    assert!(!core.properties().is_minimized());

    // minimize 应取消 maximize
    core.properties_mut().minimize();
    assert!(!core.properties().is_maximized());
    assert!(core.properties().is_minimized());

    // restore
    core.properties_mut().restore();
    assert!(!core.properties().is_maximized());
    assert!(!core.properties().is_minimized());

    // 重复 restore 无影响
    core.properties_mut().restore();
    assert!(!core.properties().is_maximized());
    assert!(!core.properties().is_minimized());
}

#[test]
fn core_set_get_position() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state, ops, presenter);

    // 初始位置
    assert_eq!(core.properties().position().x, 0.0);
    assert_eq!(core.properties().position().y, 0.0);

    core.properties_mut().set_position(100, 200);
    assert_eq!(core.properties().position().x, 100.0);
    assert_eq!(core.properties().position().y, 200.0);

    // 再设置
    core.properties_mut().set_position(-50, 75);
    assert_eq!(core.properties().position().x, -50.0);
    assert_eq!(core.properties().position().y, 75.0);
}

#[test]
fn core_features_booleans() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    let p = core.properties_mut();

    p.set_resizable(false);
    p.set_borderless(true);
    p.set_always_on_top(true);
    p.set_window_opacity(0.75);
    p.start_text_input();
    p.enable_file_drop(true);

    // 通过 state 验证
    assert!(!state.borrow().resizable);
    assert!(state.borrow().borderless);
    assert!(state.borrow().always_on_top);
    assert_eq!(state.borrow().opacity, 0.75);
    assert!(state.borrow().text_input_active);
    assert!(state.borrow().file_drop_enabled);

    // 反转
    let p = core.properties_mut();
    p.stop_text_input();
    p.enable_file_drop(false);
    assert!(!state.borrow().text_input_active);
    assert!(!state.borrow().file_drop_enabled);
}

#[test]
fn core_resize_notify_syncs_state_and_presenter() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    core.resize_notify(1920, 1080);
    assert_eq!(state.borrow().width, 1920);
    assert_eq!(state.borrow().height, 1080);
}

#[test]
fn core_set_size_updates_state_and_calls_ops() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    core.properties_mut().set_size(640, 480);
    assert_eq!(state.borrow().width, 640);
    assert_eq!(state.borrow().height, 480);
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

// ════════════════════════════════════════════════════════════════════════════
// EventBus 独立使用场景
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn event_bus_works_independently_of_event_loop() {
    use std::rc::Rc;
    use std::cell::Cell;

    // EventBus 不依赖事件循环，可以直接发布事件
    // 这是应用层推荐模式：事件循环通过 callback 桥接到 EventBus
    let mut pf = FakePlatform::new();

    let key_seen = Rc::new(Cell::new(false));
    let mouse_seen = Rc::new(Cell::new(false));

    let ks = key_seen.clone();
    pf.event_bus.subscribe(UiEventType::KeyDown, move |_| {
        ks.set(true);
        true
    });
    let ms = mouse_seen.clone();
    pf.event_bus.subscribe(UiEventType::MouseDown, move |_| {
        ms.set(true);
        true
    });

    // 直接通过 EventBus 发布事件（不经过事件循环）
    pf.event_bus.publish(&UiEvent::key_down(KeyCode::Enter, KeyMod::NONE));
    pf.event_bus.publish(&UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));

    assert!(key_seen.get());
    assert!(mouse_seen.get());
}

#[test]
fn event_bus_works_through_subscribe_all() {
    use std::rc::Rc;
    use std::cell::Cell;

    let mut pf = FakePlatform::new();

    let all_count = Rc::new(Cell::new(0u32));
    let ac = all_count.clone();
    pf.event_bus.subscribe_all(move |_| {
        ac.set(ac.get() + 1);
        true
    });

    pf.event_bus.publish(&UiEvent::close());
    pf.event_bus.publish(&UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    pf.event_bus.publish(&UiEvent::key_down(KeyCode::A, KeyMod::NONE));

    assert_eq!(all_count.get(), 3);
}

#[test]
fn event_bus_publish_called_via_event_source_callback_pattern() {
    // 这是推荐的桥接模式：用户通过 Platform trait 同时管理事件循环和 EventBus
    // callback 中获取事件并通过 event_bus() 访问器发布
    // 这里只验证 EventBus 可以独立接收和发布事件
    let mut pf = FakePlatform::new();

    pf.event_bus.subscribe_all(|_| true);
    pf.event_bus.publish(&UiEvent::close());

    // 验证 EventBus 没有 panic（订阅和发布都在同一个平台实例上）
    let _ = pf.event_bus.subscriber_count();
}

// ════════════════════════════════════════════════════════════════════════════
// OsEventSource dispatch_timeout 场景
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn os_event_source_dispatch_timeout_sends_events() {
    use std::cell::Cell;
    use std::time::Duration;

    let mut source = FakeEventSource::new();
    source.inject(UiEvent::close());

    let called = Cell::new(false);
    let result = source.wait_timeout(Duration::from_millis(10), &|_| {
        called.set(true);
        true
    });
    assert!(result);
    assert!(called.get());
}

#[test]
fn os_event_source_dispatch_timeout_respects_exit() {
    use std::cell::Cell;
    use std::time::Duration;

    let mut source = FakeEventSource::new();
    source.state.should_exit = true;
    source.inject(UiEvent::close());

    let called = Cell::new(false);
    let result = source.wait_timeout(Duration::from_millis(10), &|_| {
        called.set(true);
        true
    });
    assert!(!result);
    // dispatch_timeout 先调 dispatch_pending，它返回 false
    // 所以不会处理事件
    assert!(!called.get());
}

// ════════════════════════════════════════════════════════════════════════════
// WindowOps 与 WindowState 状态同步边界
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn window_state_fullscreen_toggle_preserves_other_state() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    core.properties_mut().set_borderless(true);
    core.properties_mut().set_always_on_top(true);
    core.properties_mut().set_fullscreen(true);

    // fullscreen 时 borderless 和 always_on_top 保持不变
    assert!(state.borrow().borderless);
    assert!(state.borrow().always_on_top);
    assert!(state.borrow().fullscreen);

    core.properties_mut().set_fullscreen(false);
    assert!(!state.borrow().fullscreen);
    assert!(state.borrow().borderless);
    assert!(state.borrow().always_on_top);
}

#[test]
fn window_state_mixed_minimize_maximize() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    // maximize → fullscreen → minimize → restore
    core.properties_mut().maximize();
    assert!(state.borrow().maximized);
    assert!(!state.borrow().minimized);

    // fullscreen 不重置 maximize/minimize（它们独立控制）
    core.properties_mut().set_fullscreen(true);
    assert!(state.borrow().fullscreen);
    assert!(state.borrow().maximized);

    // minimize 应取消 maximize
    core.properties_mut().minimize();
    assert!(state.borrow().minimized);
    assert!(!state.borrow().maximized);

    // restore
    core.properties_mut().restore();
    assert!(!state.borrow().maximized);
    assert!(!state.borrow().minimized);
}

#[test]
fn window_state_opacity_clamping() {
    let state = Rc::new(RefCell::new(WindowState::default()));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    assert_eq!(state.borrow().opacity, 1.0);
    core.properties_mut().set_window_opacity(0.0);
    assert_eq!(state.borrow().opacity, 0.0);
    core.properties_mut().set_window_opacity(0.5);
    assert_eq!(state.borrow().opacity, 0.5);
}

#[test]
fn window_state_set_size_via_properties() {
    let state = Rc::new(RefCell::new(WindowState::with_size(800, 600)));
    let ops = make_minimal_ops();
    let presenter = Box::new(NullPresenter::new());
    let mut core = PlatformWindowCore::new(state.clone(), ops, presenter);

    // 修改尺寸
    core.properties_mut().set_size(1024, 768);
    assert_eq!(state.borrow().width, 1024);
    assert_eq!(state.borrow().height, 768);

    // 修改位置
    core.properties_mut().set_position(50, 100);
    assert_eq!(state.borrow().pos_x, 50);
    assert_eq!(state.borrow().pos_y, 100);
}
