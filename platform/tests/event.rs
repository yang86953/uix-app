//! uix-platform crate 集成测试（event 模块）。

use uix_platform::event::{
    FileDropData, KeyEventData, KeyPressData, MouseButtonEventData, MouseMoveEventData,
    MouseWheelData, ResizeData, TimerEventData, UiEvent, UiEventPayload, UiEventType,
};
use uix_platform::geometry::Point;
use uix_platform::types::{KeyCode, KeyMod, MouseButton};

// ════════════════════════════════════════════════════════════════════════════
// 工厂方法测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn factory_close() {
    let ev = UiEvent::close();
    assert_eq!(ev.type_, UiEventType::WindowClose);
    assert_eq!(ev.payload, UiEventPayload::None);
}

#[test]
fn factory_mouse_down() {
    let pos = Point::new(10.0, 20.0);
    let ev = UiEvent::mouse_down(pos, MouseButton::Left);
    assert_eq!(ev.type_, UiEventType::MouseDown);
    if let UiEventPayload::MouseButton(data) = &ev.payload {
        assert_eq!(data.pos, pos);
        assert_eq!(data.btn, MouseButton::Left);
        assert_eq!(data.mods, KeyMod::NONE);
    } else {
        panic!("期望 MouseButton 载荷");
    }
}

#[test]
fn factory_mouse_up() {
    let pos = Point::new(30.0, 40.0);
    let ev = UiEvent::mouse_up(pos, MouseButton::Right);
    assert_eq!(ev.type_, UiEventType::MouseUp);
    if let UiEventPayload::MouseButton(data) = &ev.payload {
        assert_eq!(data.pos, pos);
        assert_eq!(data.btn, MouseButton::Right);
    } else {
        panic!("期望 MouseButton 载荷");
    }
}

#[test]
fn factory_mouse_move() {
    let pos = Point::new(50.0, 60.0);
    let ev = UiEvent::mouse_move(pos);
    assert_eq!(ev.type_, UiEventType::MouseMove);
    if let UiEventPayload::MouseMove(data) = &ev.payload {
        assert_eq!(data.pos, pos);
        assert_eq!(data.mods, KeyMod::NONE);
    } else {
        panic!("期望 MouseMove 载荷");
    }
}

#[test]
fn factory_mouse_wheel() {
    let pos = Point::new(70.0, 80.0);
    let ev = UiEvent::mouse_wheel(pos, 1.5, -2.5, KeyMod::CTRL);
    assert_eq!(ev.type_, UiEventType::MouseWheel);
    if let UiEventPayload::MouseWheel(data) = &ev.payload {
        assert_eq!(data.pos, pos);
        assert_eq!(data.delta_x, 1.5);
        assert_eq!(data.delta_y, -2.5);
        assert!(data.mods.contains(KeyMod::CTRL));
    } else {
        panic!("期望 MouseWheel 载荷");
    }
}

#[test]
fn factory_key_down() {
    let ev = UiEvent::key_down(KeyCode::Enter, KeyMod::SHIFT);
    assert_eq!(ev.type_, UiEventType::KeyDown);
    if let UiEventPayload::Key(data) = &ev.payload {
        assert_eq!(data.key, KeyCode::Enter);
        assert!(data.mods.contains(KeyMod::SHIFT));
    } else {
        panic!("期望 Key 载荷");
    }
}

#[test]
fn factory_key_up() {
    let ev = UiEvent::key_up(KeyCode::Escape, KeyMod::NONE);
    assert_eq!(ev.type_, UiEventType::KeyUp);
    if let UiEventPayload::Key(data) = &ev.payload {
        assert_eq!(data.key, KeyCode::Escape);
        assert_eq!(data.mods, KeyMod::NONE);
    } else {
        panic!("期望 Key 载荷");
    }
}

#[test]
fn factory_key_press() {
    let ev = UiEvent::key_press("hello");
    assert_eq!(ev.type_, UiEventType::KeyPress);
    if let UiEventPayload::KeyPress(data) = &ev.payload {
        assert_eq!(data.text, "hello");
    } else {
        panic!("期望 KeyPress 载荷");
    }
}

#[test]
fn factory_key_press_empty_string() {
    let ev = UiEvent::key_press("");
    assert_eq!(ev.type_, UiEventType::KeyPress);
    if let UiEventPayload::KeyPress(data) = &ev.payload {
        assert_eq!(data.text, "");
    } else {
        panic!("期望 KeyPress 载荷");
    }
}

#[test]
fn factory_timer() {
    let ev = UiEvent::timer(42);
    assert_eq!(ev.type_, UiEventType::Timer);
    if let UiEventPayload::Timer(data) = &ev.payload {
        assert_eq!(data.timer_id, 42);
    } else {
        panic!("期望 Timer 载荷");
    }
}

#[test]
fn factory_timer_zero_id() {
    let ev = UiEvent::timer(0);
    assert_eq!(ev.type_, UiEventType::Timer);
    if let UiEventPayload::Timer(data) = &ev.payload {
        assert_eq!(data.timer_id, 0);
    } else {
        panic!("期望 Timer 载荷");
    }
}

#[test]
fn factory_resize() {
    let ev = UiEvent::resize(800, 600);
    assert_eq!(ev.type_, UiEventType::WindowResize);
    if let UiEventPayload::Resize(data) = &ev.payload {
        assert_eq!(data.width, 800);
        assert_eq!(data.height, 600);
    } else {
        panic!("期望 Resize 载荷");
    }
}

#[test]
fn factory_resize_zero() {
    let ev = UiEvent::resize(0, 0);
    assert_eq!(ev.type_, UiEventType::WindowResize);
    if let UiEventPayload::Resize(data) = &ev.payload {
        assert_eq!(data.width, 0);
        assert_eq!(data.height, 0);
    } else {
        panic!("期望 Resize 载荷");
    }
}

#[test]
fn factory_file_drop() {
    let pos = Point::new(100.0, 200.0);
    let files = vec!["a.txt".into(), "b.txt".into()];
    let ev = UiEvent::file_drop(files.clone(), pos);
    assert_eq!(ev.type_, UiEventType::FileDrop);
    if let UiEventPayload::FileDrop(data) = &ev.payload {
        assert_eq!(data.files, files);
        assert_eq!(data.position, pos);
    } else {
        panic!("期望 FileDrop 载荷");
    }
}

#[test]
fn factory_file_drop_empty() {
    let pos = Point::new(0.0, 0.0);
    let ev = UiEvent::file_drop(vec![], pos);
    assert_eq!(ev.type_, UiEventType::FileDrop);
    if let UiEventPayload::FileDrop(data) = &ev.payload {
        assert!(data.files.is_empty());
        assert_eq!(data.position, pos);
    } else {
        panic!("期望 FileDrop 载荷");
    }
}

#[test]
fn factory_new_generic() {
    let ev = UiEvent::new(UiEventType::WindowMinimize, UiEventPayload::None);
    assert_eq!(ev.type_, UiEventType::WindowMinimize);
    assert_eq!(ev.payload, UiEventPayload::None);
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent 字段直接构造
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn event_manual_construction() {
    let ev = UiEvent {
        type_: UiEventType::WindowFocus,
        payload: UiEventPayload::None,
    };
    assert_eq!(ev.type_, UiEventType::WindowFocus);
}

// ════════════════════════════════════════════════════════════════════════════
// 载荷匹配
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn payload_match_none() {
    let ev = UiEvent::close();
    assert!(matches!(ev.payload, UiEventPayload::None));
}

#[test]
fn payload_match_key() {
    let ev = UiEvent::key_down(KeyCode::A, KeyMod::CTRL);
    assert!(matches!(ev.payload, UiEventPayload::Key(_)));
}

#[test]
fn payload_match_mouse_button() {
    let ev = UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left);
    assert!(matches!(ev.payload, UiEventPayload::MouseButton(_)));
}

#[test]
fn payload_match_mouse_move() {
    let ev = UiEvent::mouse_move(Point::new(1.0, 2.0));
    assert!(matches!(ev.payload, UiEventPayload::MouseMove(_)));
}

#[test]
fn payload_match_mouse_wheel() {
    let ev = UiEvent::mouse_wheel(Point::new(0.0, 0.0), 0.0, 1.0, KeyMod::NONE);
    assert!(matches!(ev.payload, UiEventPayload::MouseWheel(_)));
}

#[test]
fn payload_match_resize() {
    let ev = UiEvent::resize(100, 200);
    assert!(matches!(ev.payload, UiEventPayload::Resize(_)));
}

#[test]
fn payload_match_timer() {
    let ev = UiEvent::timer(1);
    assert!(matches!(ev.payload, UiEventPayload::Timer(_)));
}

#[test]
fn payload_match_key_press() {
    let ev = UiEvent::key_press("x");
    assert!(matches!(ev.payload, UiEventPayload::KeyPress(_)));
}

#[test]
fn payload_match_file_drop() {
    let ev = UiEvent::file_drop(vec![], Point::new(0.0, 0.0));
    assert!(matches!(ev.payload, UiEventPayload::FileDrop(_)));
}

#[test]
fn payload_match_if_let_pattern() {
    let ev = UiEvent::key_down(KeyCode::Space, KeyMod::NONE);
    if let UiEventPayload::Key(data) = &ev.payload {
        assert_eq!(data.key, KeyCode::Space);
    } else {
        panic!("if let 模式匹配失败");
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 事件数据结构体默认实现
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn key_event_data_default() {
    let data = KeyEventData::default();
    assert_eq!(data.key, KeyCode::Unknown);
    assert_eq!(data.mods, KeyMod::NONE);
}

#[test]
fn mouse_button_event_data_default() {
    let data = MouseButtonEventData::default();
    assert_eq!(data.pos, Point::default());
    assert_eq!(data.btn, MouseButton::None);
    assert_eq!(data.mods, KeyMod::NONE);
}

#[test]
fn mouse_move_event_data_default() {
    let data = MouseMoveEventData::default();
    assert_eq!(data.pos, Point::default());
    assert_eq!(data.mods, KeyMod::NONE);
}

#[test]
fn mouse_wheel_data_default() {
    let data = MouseWheelData::default();
    assert_eq!(data.pos, Point::default());
    assert_eq!(data.delta_x, 0.0);
    assert_eq!(data.delta_y, 0.0);
    assert_eq!(data.mods, KeyMod::NONE);
}

#[test]
fn resize_data_default() {
    let data = ResizeData::default();
    assert_eq!(data.width, 0);
    assert_eq!(data.height, 0);
}

#[test]
fn timer_event_data_default() {
    let data = TimerEventData::default();
    assert_eq!(data.timer_id, 0);
}

#[test]
fn key_press_data_default() {
    let data = KeyPressData::default();
    assert_eq!(data.text, "");
}

#[test]
fn file_drop_data_default() {
    let data = FileDropData::default();
    assert!(data.files.is_empty());
    assert_eq!(data.position, Point::default());
}

// ════════════════════════════════════════════════════════════════════════════
// UiEventPayload Default
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn payload_default_is_none() {
    let payload = UiEventPayload::default();
    assert_eq!(payload, UiEventPayload::None);
}

// ════════════════════════════════════════════════════════════════════════════
// UiEventType 变体完整性
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn event_type_all_variants_have_unique_repr() {
    let variants = vec![
        (UiEventType::Unknown as u8, "Unknown"),
        (UiEventType::WindowClose as u8, "WindowClose"),
        (UiEventType::WindowResize as u8, "WindowResize"),
        (UiEventType::WindowMinimize as u8, "WindowMinimize"),
        (UiEventType::WindowMaximize as u8, "WindowMaximize"),
        (UiEventType::WindowRestore as u8, "WindowRestore"),
        (UiEventType::WindowFocus as u8, "WindowFocus"),
        (UiEventType::WindowBlur as u8, "WindowBlur"),
        (UiEventType::MouseDown as u8, "MouseDown"),
        (UiEventType::MouseUp as u8, "MouseUp"),
        (UiEventType::MouseMove as u8, "MouseMove"),
        (UiEventType::MouseWheel as u8, "MouseWheel"),
        (UiEventType::KeyDown as u8, "KeyDown"),
        (UiEventType::KeyUp as u8, "KeyUp"),
        (UiEventType::KeyPress as u8, "KeyPress"),
        (UiEventType::Timer as u8, "Timer"),
        (UiEventType::FileDrop as u8, "FileDrop"),
    ];
    let mut seen = std::collections::HashSet::new();
    for (repr, name) in &variants {
        assert!(seen.insert(repr), "duplicate repr {} for {}", repr, name);
    }
    assert_eq!(seen.len(), variants.len());
}
