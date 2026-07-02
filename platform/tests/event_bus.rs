//! uix-platform crate 集成测试（event_bus 模块）。

use uix_platform::event_bus::EventBus;
use uix_platform::event::{UiEvent, UiEventType};
use uix_platform::geometry::Point;
use uix_platform::types::{KeyCode, KeyMod, MouseButton};
use std::cell::Cell;
use std::rc::Rc;

// ════════════════════════════════════════════════════════════════════════════
// EventBus 基础测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn default_implementation_works() {
    let bus = EventBus::default();
    assert_eq!(bus.subscriber_count(), 0);
}

#[test]
fn new_creates_empty_bus() {
    let bus = EventBus::new();
    assert_eq!(bus.subscriber_count(), 0);
}

// ════════════════════════════════════════════════════════════════════════════
// 订阅 / 发布
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn subscribe_and_publish_matching_event_handler_called() {
    let mut bus = EventBus::new();
    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    bus.subscribe(UiEventType::MouseDown, move |_| {
        c.set(true);
        true
    });
    let ev = UiEvent::mouse_down(Point::new(10.0, 10.0), MouseButton::Left);
    bus.publish(&ev);
    assert!(called.get());
}

#[test]
fn subscribe_publish_non_matching_event_handler_not_called() {
    let mut bus = EventBus::new();
    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    bus.subscribe(UiEventType::KeyDown, move |_| {
        c.set(true);
        true
    });
    let ev = UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left);
    bus.publish(&ev);
    assert!(!called.get());
}

#[test]
fn subscribe_all_handler_called_for_all_types() {
    let mut bus = EventBus::new();
    let count = Rc::new(Cell::new(0u32));
    let c = count.clone();
    bus.subscribe_all(move |_| {
        c.set(c.get() + 1);
        true
    });
    bus.publish(&UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    bus.publish(&UiEvent::key_down(KeyCode::Enter, KeyMod::NONE));
    bus.publish(&UiEvent::close());
    assert_eq!(count.get(), 3);
}

#[test]
fn publish_returns_true_when_all_handlers_return_true() {
    let mut bus = EventBus::new();
    bus.subscribe(UiEventType::MouseDown, |_| true);
    bus.subscribe(UiEventType::MouseDown, |_| true);
    let result = bus.publish(&UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    assert!(result);
}

#[test]
fn publish_returns_false_when_handler_stops() {
    let mut bus = EventBus::new();
    bus.subscribe(UiEventType::MouseDown, |_| false);
    let result = bus.publish(&UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    assert!(!result);
}

// ════════════════════════════════════════════════════════════════════════════
// 传播控制
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn handler_returning_false_stops_propagation_to_later_handlers() {
    let mut bus = EventBus::new();
    let handler2_called = Rc::new(Cell::new(false));
    let h2 = handler2_called.clone();
    let handler3_called = Rc::new(Cell::new(false));
    let h3 = handler3_called.clone();
    bus.subscribe(UiEventType::MouseDown, move |_| true);
    bus.subscribe(UiEventType::MouseDown, move |_| {
        h2.set(true);
        false
    });
    bus.subscribe(UiEventType::MouseDown, move |_| {
        h3.set(true);
        true
    });
    let ev = UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left);
    let result = bus.publish(&ev);
    assert!(!result);
    assert!(handler2_called.get());
    assert!(!handler3_called.get());
}

#[test]
fn multiple_subscribers_receive_events_in_order() {
    let mut bus = EventBus::new();
    let order = Rc::new(Cell::new(0u32));
    let o1 = order.clone();
    bus.subscribe(UiEventType::MouseDown, move |_| {
        assert_eq!(o1.get(), 0);
        o1.set(1);
        true
    });
    let o2 = order.clone();
    bus.subscribe(UiEventType::MouseDown, move |_| {
        assert_eq!(o2.get(), 1);
        o2.set(2);
        true
    });
    let ev = UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left);
    bus.publish(&ev);
    assert_eq!(order.get(), 2);
}

// ════════════════════════════════════════════════════════════════════════════
// 取消订阅 / 清空
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn unsubscribe_handler_no_longer_called() {
    let mut bus = EventBus::new();
    let called = Rc::new(Cell::new(false));
    let c = called.clone();
    let id = bus.subscribe(UiEventType::MouseDown, move |_| {
        c.set(true);
        true
    });
    bus.unsubscribe(id);
    bus.publish(&UiEvent::mouse_down(Point::new(0.0, 0.0), MouseButton::Left));
    assert!(!called.get());
}

#[test]
fn unsubscribe_twice_is_harmless() {
    let mut bus = EventBus::new();
    let id = bus.subscribe(UiEventType::MouseDown, |_| true);
    bus.unsubscribe(id);
    bus.unsubscribe(id);
    assert_eq!(bus.subscriber_count(), 0);
}

#[test]
fn unsubscribe_unknown_id_does_nothing() {
    let mut bus = EventBus::new();
    bus.subscribe(UiEventType::MouseDown, |_| true);
    bus.unsubscribe(999);
    assert_eq!(bus.subscriber_count(), 1);
}

#[test]
fn clear_removes_all_subscribers() {
    let mut bus = EventBus::new();
    let called1 = Rc::new(Cell::new(false));
    let called2 = Rc::new(Cell::new(false));
    let c1 = called1.clone();
    let c2 = called2.clone();
    bus.subscribe(UiEventType::KeyDown, move |_| {
        c1.set(true);
        true
    });
    bus.subscribe_all(move |_| {
        c2.set(true);
        true
    });
    bus.clear();
    bus.publish(&UiEvent::close());
    assert!(!called1.get());
    assert!(!called2.get());
    assert_eq!(bus.subscriber_count(), 0);
}

#[test]
fn clear_empty_bus_does_nothing() {
    let mut bus = EventBus::new();
    bus.clear();
    assert_eq!(bus.subscriber_count(), 0);
}

// ════════════════════════════════════════════════════════════════════════════
// 订阅计数
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn subscriber_count_starts_at_zero() {
    let bus = EventBus::new();
    assert_eq!(bus.subscriber_count(), 0);
}

#[test]
fn subscriber_count_increases_on_subscribe() {
    let mut bus = EventBus::new();
    bus.subscribe(UiEventType::MouseDown, |_| true);
    assert_eq!(bus.subscriber_count(), 1);
}

#[test]
fn subscriber_count_increases_on_subscribe_all() {
    let mut bus = EventBus::new();
    bus.subscribe_all(|_| true);
    assert_eq!(bus.subscriber_count(), 1);
}

#[test]
fn subscriber_count_decreases_on_unsubscribe() {
    let mut bus = EventBus::new();
    let id1 = bus.subscribe(UiEventType::MouseDown, |_| true);
    bus.subscribe(UiEventType::KeyDown, |_| true);
    assert_eq!(bus.subscriber_count(), 2);
    bus.unsubscribe(id1);
    assert_eq!(bus.subscriber_count(), 1);
}

#[test]
fn subscriber_count_multiple_subscriptions() {
    let mut bus = EventBus::new();
    for _ in 0..5 {
        bus.subscribe(UiEventType::MouseDown, |_| true);
    }
    assert_eq!(bus.subscriber_count(), 5);
}
