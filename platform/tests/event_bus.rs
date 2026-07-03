//! uix-platform crate 集成测试（event_bus 模块）。

use std::cell::Cell;
use std::rc::Rc;
use uix_platform::event::{UiEvent, UiEventType};
use uix_platform::event_bus::EventBus;
use uix_platform::geometry::Point;
use uix_platform::types::{KeyCode, KeyMod, MouseButton};

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
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    bus.publish(&UiEvent::key_down(KeyCode::Enter, KeyMod::NONE));
    bus.publish(&UiEvent::close());
    assert_eq!(count.get(), 3);
}

#[test]
fn publish_returns_true_when_all_handlers_return_true() {
    let mut bus = EventBus::new();
    bus.subscribe(UiEventType::MouseDown, |_| true);
    bus.subscribe(UiEventType::MouseDown, |_| true);
    let result = bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    assert!(result);
}

#[test]
fn publish_returns_false_when_handler_stops() {
    let mut bus = EventBus::new();
    bus.subscribe(UiEventType::MouseDown, |_| false);
    let result = bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
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
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
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
// 一次性订阅
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn subscribe_once_triggers_and_auto_removes() {
    let mut bus = EventBus::new();
    let call_count = Rc::new(Cell::new(0u32));
    let c = call_count.clone();

    bus.subscribe_once(UiEventType::MouseDown, move |_| {
        c.set(c.get() + 1);
        true
    });

    // 第一次发布：触发，自动移除
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    assert_eq!(call_count.get(), 1);

    // 第二次发布：不再触发
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    assert_eq!(call_count.get(), 1);
}

#[test]
fn subscribe_once_only_fires_for_matching_type() {
    let mut bus = EventBus::new();
    let call_count = Rc::new(Cell::new(0u32));
    let c = call_count.clone();

    bus.subscribe_once(UiEventType::KeyDown, move |_| {
        c.set(c.get() + 1);
        true
    });

    // 不匹配的事件：不触发，不移除
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    assert_eq!(call_count.get(), 0);

    // 匹配的事件：触发并移除
    bus.publish(&UiEvent::key_down(KeyCode::Enter, KeyMod::NONE));
    assert_eq!(call_count.get(), 1);
}

#[test]
fn subscribe_all_once_triggers_for_any_event() {
    let mut bus = EventBus::new();
    let call_count = Rc::new(Cell::new(0u32));
    let c = call_count.clone();

    bus.subscribe_all_once(move |_| {
        c.set(c.get() + 1);
        true
    });

    // 首次任意事件触发并移除
    bus.publish(&UiEvent::close());
    assert_eq!(call_count.get(), 1);

    // 后续不触发
    bus.publish(&UiEvent::close());
    assert_eq!(call_count.get(), 1);
}

#[test]
fn subscribe_once_handler_returning_false_stops_propagation() {
    let mut bus = EventBus::new();
    let handler2_called = Rc::new(Cell::new(false));
    let h2 = handler2_called.clone();

    bus.subscribe_once(UiEventType::MouseDown, |_| false);
    bus.subscribe(UiEventType::MouseDown, move |_| {
        h2.set(true);
        true
    });

    let result = bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    assert!(!result, "publish should return false");
    // 一次性返回 false 后，第二个 handler 不被调用
    assert!(!handler2_called.get());
}

// ════════════════════════════════════════════════════════════════════════════
// 优先级订阅
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn higher_priority_runs_first() {
    let mut bus = EventBus::new();
    let order = Rc::new(Cell::new(Vec::<u32>::new()));
    let o1 = order.clone();
    let o2 = order.clone();
    let o3 = order.clone();

    bus.subscribe_with_priority(UiEventType::MouseDown, 0, move |_| {
        let mut v = o1.take();
        v.push(1);
        o1.set(v);
        true
    });
    bus.subscribe_with_priority(UiEventType::MouseDown, 10, move |_| {
        let mut v = o2.take();
        v.push(2);
        o2.set(v);
        true
    });
    bus.subscribe_with_priority(UiEventType::MouseDown, -5, move |_| {
        let mut v = o3.take();
        v.push(3);
        o3.set(v);
        true
    });

    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));

    let v = order.take();
    // 优先级 10 > 0 > -5
    assert_eq!(
        v,
        vec![2, 1, 3],
        "should execute in priority order: 10, 0, -5"
    );
}

#[test]
fn same_priority_keeps_registration_order() {
    let mut bus = EventBus::new();
    let order = Rc::new(Cell::new(Vec::<u32>::new()));
    let o1 = order.clone();
    let o2 = order.clone();
    let o3 = order.clone();

    bus.subscribe_with_priority(UiEventType::MouseDown, 0, move |_| {
        let mut v = o1.take();
        v.push(1);
        o1.set(v);
        true
    });
    bus.subscribe_with_priority(UiEventType::MouseDown, 0, move |_| {
        let mut v = o2.take();
        v.push(2);
        o2.set(v);
        true
    });
    bus.subscribe_with_priority(UiEventType::MouseDown, 0, move |_| {
        let mut v = o3.take();
        v.push(3);
        o3.set(v);
        true
    });

    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));

    let v = order.take();
    assert_eq!(
        v,
        vec![1, 2, 3],
        "same priority should keep registration order"
    );
}

#[test]
fn high_priority_once_fires_before_low_priority() {
    let mut bus = EventBus::new();
    let order = Rc::new(Cell::new(Vec::<u32>::new()));
    let o1 = order.clone();
    let o2 = order.clone();

    // 高优先级 + 一次性
    bus.subscribe_once_with_priority(UiEventType::MouseDown, 100, move |_| {
        let mut v = o1.take();
        v.push(1);
        o1.set(v);
        true
    });
    // 低优先级
    bus.subscribe_with_priority(UiEventType::MouseDown, -100, move |_| {
        let mut v = o2.take();
        v.push(2);
        o2.set(v);
        true
    });

    // 第一次：两者都触发
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    let v = order.take();
    assert_eq!(v, vec![1, 2], "high priority once should fire first");

    // 第二次：只剩低优先级（一次性已移除）
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    let v = order.take();
    assert_eq!(v, vec![2], "only low priority should remain");
}

#[test]
fn subscribe_all_with_priority_works() {
    let mut bus = EventBus::new();
    let count = Rc::new(Cell::new(0u32));
    let c = count.clone();

    bus.subscribe_all_with_priority(50, move |_| {
        c.set(c.get() + 1);
        true
    });

    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    ));
    assert_eq!(count.get(), 1);

    bus.publish(&UiEvent::key_down(KeyCode::Enter, KeyMod::NONE));
    assert_eq!(count.get(), 2);
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

#[test]
fn subscribe_once_removed_from_count() {
    let mut bus = EventBus::new();
    bus.subscribe_once(UiEventType::MouseDown, |_| true);
    assert_eq!(bus.subscriber_count(), 1);
    bus.publish(&UiEvent::close()); // 不匹配，不移除
    assert_eq!(bus.subscriber_count(), 1);
    bus.publish(&UiEvent::mouse_down(
        Point::new(0.0, 0.0),
        MouseButton::Left,
    )); // 匹配，触发并移除
    assert_eq!(bus.subscriber_count(), 0);
}
