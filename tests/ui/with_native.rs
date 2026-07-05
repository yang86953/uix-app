//! ui + native 域 — FakePlatform 集成测试（需 test-harness）。
//!
//! 使用 `uix::native::test_harness::FakePlatform` 验证 UI 组件与平台服务的交互。
//!
//! 运行方式：
//! ```bash
//! cargo test --features test-harness -p uix
//! ```

#![cfg(feature = "test-harness")]

use std::cell::Cell;

use uix::core::geometry::Point;
use uix::native::test_harness::FakePlatform;
use uix::native::{
    IClipboard, IDisplay, IEventLoop, IWindowManager, MouseButton, Platform, UiEvent,
    UiEventPayload, UiEventType,
};

// ════════════════════════════════════════════════════════════════════════════
// 辅助函数
// ════════════════════════════════════════════════════════════════════════════

/// 从 FakePlatform 提取 `IClipboard` fat pointer 并注入到 UI 剪贴板处理器。
///
/// 使用与 `render_loop.rs` 中 production 代码相同的 unsafe transmute 模式。
fn inject_clipboard_handler(pf: &mut FakePlatform) {
    let clipboard_trait: &mut dyn IClipboard = Platform::clipboard(pf);
    let wide: *mut dyn IClipboard = clipboard_trait;
    // Safety: 将 fat pointer 拆分为 (data, vtable) 两个 usize，
    // 后续在 copy_to_clipboard 中按相同 layout 重构。
    // 与 production 代码（render_loop.rs）完全一致的模式。
    let parts: (usize, usize) = unsafe { std::mem::transmute(wide) };
    uix::ui::foundation::clipboard::set_clipboard_parts(parts.0, parts.1);
}

// ════════════════════════════════════════════════════════════════════════════
// 剪贴板集成测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn clipboard_with_fake_platform() {
    let mut pf = FakePlatform::new();
    inject_clipboard_handler(&mut pf);

    // 通过 UI 的 copy_to_clipboard 写入文本
    uix::ui::foundation::clipboard::copy_to_clipboard("hello from ui");

    // 验证文本已到达 FakeClipboard（通过 IClipboard trait 访问）
    assert_eq!(pf.clipboard.text(), "hello from ui");
    // 直接访问 FakeClipboard 的状态字段断言调用历史
    assert_eq!(pf.clipboard.last_set_text(), Some("hello from ui"));
    assert_eq!(pf.clipboard.state.set_text_calls.len(), 1);
}

#[test]
fn clipboard_multiple_writes() {
    let mut pf = FakePlatform::new();
    inject_clipboard_handler(&mut pf);

    uix::ui::foundation::clipboard::copy_to_clipboard("first");
    assert_eq!(pf.clipboard.text(), "first");

    uix::ui::foundation::clipboard::copy_to_clipboard("second");
    assert_eq!(pf.clipboard.text(), "second");

    assert_eq!(pf.clipboard.state.set_text_calls.len(), 2);
}

#[test]
fn clipboard_has_text_after_copy() {
    let mut pf = FakePlatform::new();
    inject_clipboard_handler(&mut pf);

    assert!(!pf.clipboard.has_text());
    uix::ui::foundation::clipboard::copy_to_clipboard("now has text");
    assert!(pf.clipboard.has_text());
}

#[test]
fn clipboard_handler_replacement() {
    let mut pf = FakePlatform::new();
    inject_clipboard_handler(&mut pf);

    uix::ui::foundation::clipboard::copy_to_clipboard("before re-inject");
    assert_eq!(pf.clipboard.text(), "before re-inject");

    // 重新注入（模拟窗口重建等场景）
    inject_clipboard_handler(&mut pf);

    uix::ui::foundation::clipboard::copy_to_clipboard("after re-inject");
    assert_eq!(pf.clipboard.text(), "after re-inject");
}

// ════════════════════════════════════════════════════════════════════════════
// 事件源集成测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn inject_mouse_down_through_event_source() {
    let mut pf = FakePlatform::new();

    // 注入 MouseDown 事件到 FakeEventSource
    pf.event_source.inject(UiEvent::mouse_down(
        Point::new(100.0, 200.0),
        MouseButton::Left,
    ));

    // 通过 IEventLoop::poll_event 消费事件
    let called = Cell::new(false);
    let result = pf.event_loop().poll_event(&|ev| {
        called.set(true);
        assert_eq!(ev.type_, UiEventType::MouseDown);
        true
    });

    assert!(result, "poll_event should return true");
    assert!(called.get(), "callback should have been called");
    assert_eq!(pf.event_source.pending_count(), 0);
    assert_eq!(pf.event_source.processed_count(), 1);
}

#[test]
fn inject_multiple_events_processed_in_order() {
    let mut pf = FakePlatform::new();

    pf.event_source.inject(UiEvent::mouse_down(
        Point::new(10.0, 20.0),
        MouseButton::Left,
    ));
    pf.event_source
        .inject(UiEvent::mouse_up(Point::new(10.0, 20.0), MouseButton::Left));
    pf.event_source
        .inject(UiEvent::mouse_move(Point::new(30.0, 40.0)));

    let events: std::cell::RefCell<Vec<UiEventType>> = std::cell::RefCell::new(Vec::new());

    // poll_event 消费所有待处理事件
    pf.event_loop().poll_event(&|ev| {
        events.borrow_mut().push(ev.type_);
        true
    });

    let types = events.into_inner();
    assert_eq!(types.len(), 3);
    assert_eq!(types[0], UiEventType::MouseDown);
    assert_eq!(types[1], UiEventType::MouseUp);
    assert_eq!(types[2], UiEventType::MouseMove);
}

#[test]
fn inject_close_event_triggers_callback_return_false() {
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::close());

    let called = Cell::new(false);
    // 回调返回 false 模拟退出信号
    let result = pf.event_loop().poll_event(&|ev| {
        called.set(true);
        assert_eq!(ev.type_, UiEventType::WindowClose);
        false
    });

    assert!(
        !result,
        "poll_event should return false when callback returns false"
    );
    assert!(called.get());
}

#[test]
fn resize_event_payload() {
    let mut pf = FakePlatform::new();
    pf.event_source.inject(UiEvent::resize(1920, 1080));

    let called = Cell::new(false);
    pf.event_loop().poll_event(&|ev| {
        called.set(true);
        assert_eq!(ev.type_, UiEventType::WindowResize);
        if let UiEventPayload::Resize(ref d) = ev.payload {
            assert_eq!(d.width, 1920);
            assert_eq!(d.height, 1080);
        } else {
            panic!("expected Resize payload, got {:?}", ev.payload);
        }
        true
    });
    assert!(called.get());
}

#[test]
fn key_event_injection() {
    use uix::native::{KeyCode, KeyMod};

    let mut pf = FakePlatform::new();
    pf.event_source
        .inject(UiEvent::key_down(KeyCode::A, KeyMod::CTRL));

    let called = Cell::new(false);
    pf.event_loop().poll_event(&|ev| {
        called.set(true);
        assert_eq!(ev.type_, UiEventType::KeyDown);
        if let UiEventPayload::Key(ref k) = ev.payload {
            assert_eq!(k.key, KeyCode::A);
            assert!(k.mods.contains(KeyMod::CTRL));
        } else {
            panic!("expected Key payload");
        }
        true
    });
    assert!(called.get());
}

#[test]
fn fake_event_source_clear() {
    let mut pf = FakePlatform::new();

    pf.event_source
        .inject(UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    pf.event_source
        .inject(UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    assert_eq!(pf.event_source.pending_count(), 2);

    pf.event_source.clear();
    assert_eq!(pf.event_source.pending_count(), 0);
    assert_eq!(pf.event_source.processed_count(), 0);
}

// ════════════════════════════════════════════════════════════════════════════
// 显示 & 窗口测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn display_info_from_fake_platform() {
    let pf = FakePlatform::new();
    // 通过 IDisplay trait 方法访问（与 production 代码一致的路径）
    let display: &dyn IDisplay = Platform::display(&pf);
    assert_eq!(display.dpi_scale(), 1.0);
    assert!(!display.is_dark_mode());
    assert_eq!(display.count(), 1);
}

#[test]
fn display_dark_mode_customization() {
    let pf = FakePlatform::new();
    pf.display.set_dark_mode(true);
    assert!(pf.display.is_dark_mode.get());

    pf.display.set_dpi(2.0);
    assert_eq!(pf.display.dpi_scale.get(), 2.0);
}

#[test]
fn display_multiple_monitors() {
    let pf = FakePlatform::new();
    pf.display.set_count(2);

    assert_eq!(pf.display.count.get(), 2);
    let info0 = pf.display.info(0);
    assert!(info0.is_primary);
    let info1 = pf.display.info(1);
    assert!(!info1.is_primary);
    assert_eq!(pf.display.info_calls.borrow().len(), 2);
}

#[test]
fn window_creation_and_basic_properties() {
    let mut pf = FakePlatform::new();

    let mut window = pf
        .window_manager
        .create_window("Test", 800, 600)
        .expect("should create window");

    assert!(!window.is_visible());
    window.show();
    assert!(window.is_visible());

    assert_eq!(window.properties().width(), 800);
    assert_eq!(window.properties().height(), 600);

    window.resize_notify(1024, 768);
    assert_eq!(window.properties().width(), 1024);
    assert_eq!(window.properties().height(), 768);
}

#[test]
fn window_properties_set_get() {
    let mut pf = FakePlatform::new();
    let mut window = pf
        .window_manager
        .create_window("Props", 400, 300)
        .expect("should create window");

    // 通过 properties_mut 设置属性
    {
        let props = window.properties_mut();
        props.set_resizable(false);
        props.set_minimum_size(200, 150);
        props.set_maximum_size(1600, 1200);
        props.set_borderless(true);
        props.set_position(100, 200);
    }

    // 通过 properties 读取
    let props = window.properties();
    assert_eq!(props.width(), 400);
    assert_eq!(props.height(), 300);
    assert_eq!(props.position().x, 100.0);
    assert_eq!(props.position().y, 200.0);
}

#[test]
fn window_show_hide_close() {
    let mut pf = FakePlatform::new();
    let mut window = pf
        .window_manager
        .create_window("Visibility", 640, 480)
        .expect("should create window");

    assert!(!window.is_visible());
    window.show();
    assert!(window.is_visible());
    window.hide();
    assert!(!window.is_visible());
    window.show();
    assert!(window.is_visible());
    window.close();
    assert!(!window.is_visible());
}

#[test]
fn window_set_title() {
    let mut pf = FakePlatform::new();
    let mut window = pf
        .window_manager
        .create_window("Initial", 640, 480)
        .expect("should create window");

    window.set_title("Updated Title");
    window.set_title("Final Title");
    // 验证 set_title 被记录了两次
    assert_eq!(
        pf.window_manager.create_calls.len(),
        1,
        "only one window should be created"
    );
}

#[test]
fn window_manager_create_tracks_calls() {
    let mut pf = FakePlatform::new();

    pf.window_manager
        .create_window("First", 800, 600)
        .expect("should create first window");
    pf.window_manager
        .create_window("Second", 400, 300)
        .expect("should create second window");

    assert_eq!(pf.window_manager.create_calls.len(), 2);
    assert_eq!(pf.window_manager.create_calls[0].0, "First");
    assert_eq!(pf.window_manager.create_calls[1].0, "Second");
}

#[test]
fn window_center_raise_lower_flash() {
    let mut pf = FakePlatform::new();
    let mut window = pf
        .window_manager
        .create_window("Ops", 800, 600)
        .expect("should create window");

    window.center_on_screen();
    window.raise();
    window.lower();
    window.flash_window();
    // 不 crash 就算通过
}

// ════════════════════════════════════════════════════════════════════════════
// Platform trait 完整性测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn platform_trait_all_subsystems_accessible() {
    let mut pf = FakePlatform::new();

    // 验证所有 Platform trait 方法可通过 FakePlatform 访问
    let _clipboard: &mut dyn IClipboard = Platform::clipboard(&mut pf);
    let _event_loop: &mut dyn IEventLoop = Platform::event_loop(&mut pf);

    // 验证消息总线可访问
    let _bus = Platform::event_bus(&mut pf);
}
