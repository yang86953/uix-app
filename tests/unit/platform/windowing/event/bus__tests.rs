//! 平台输入事件分发通道的最小验证：订阅/发布、类型过滤、
//! 通配符、优先级、一次性、注销幂等、清空与计数。

use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::platform::windowing::event::types::UiEventPayload;

/// 构造一个指定类型的 UiEvent（测试负载）。
fn event_of(kind: UiEventType) -> UiEvent {
    UiEvent::new(kind, UiEventPayload::None)
}

#[test]
fn subscribe_and_publish_invokes_matching_handler() {
    // 订阅特定类型：发布同类型事件时处理器被调用。
    let mut bus = EventBus::new();
    let calls = Rc::new(RefCell::new(0u32));
    let c = Rc::clone(&calls);
    let _id = bus.subscribe(UiEventType::PointerMove, move |_| {
        *c.borrow_mut() += 1;
    });
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(*calls.borrow(), 1);
}

#[test]
fn type_filter_ignores_other_event_types() {
    // 只分发订阅的类型：其他类型不触发处理器。
    let mut bus = EventBus::new();
    let calls = Rc::new(RefCell::new(0u32));
    let c = Rc::clone(&calls);
    let _id = bus.subscribe(UiEventType::PointerMove, move |_| {
        *c.borrow_mut() += 1;
    });
    bus.publish(&event_of(UiEventType::KeyDown));
    assert_eq!(*calls.borrow(), 0);
}

#[test]
fn subscribe_all_matches_every_event() {
    // 通配符订阅匹配所有事件类型。
    let mut bus = EventBus::new();
    let calls = Rc::new(RefCell::new(0u32));
    let c = Rc::clone(&calls);
    let _id = bus.subscribe_all(move |_| {
        *c.borrow_mut() += 1;
    });
    bus.publish(&event_of(UiEventType::PointerMove));
    bus.publish(&event_of(UiEventType::KeyDown));
    assert_eq!(*calls.borrow(), 2);
}

#[test]
fn priority_order_is_highest_first() {
    // 优先级越高越先执行；同优先级按注册顺序（记录执行顺序）。
    let order = Rc::new(RefCell::new(Vec::new()));
    let mut bus = EventBus::new();
    let o1 = Rc::clone(&order);
    let _low = bus.subscribe_with_priority(UiEventType::PointerMove, PRIORITY_LOWEST, move |_| {
        o1.borrow_mut().push("low");
    });
    let o2 = Rc::clone(&order);
    let _high =
        bus.subscribe_with_priority(UiEventType::PointerMove, PRIORITY_HIGHEST, move |_| {
            o2.borrow_mut().push("high");
        });
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(*order.borrow(), ["high", "low"]);
}

#[test]
fn equal_priority_preserves_registration_order_across_publishes() {
    let mut bus = EventBus::new();
    let order = Rc::new(RefCell::new(Vec::new()));
    for value in [1, 2, 3] {
        let order = Rc::clone(&order);
        bus.subscribe_with_priority(UiEventType::PointerMove, 7, move |_| {
            order.borrow_mut().push(value);
        });
    }

    bus.publish(&event_of(UiEventType::PointerMove));
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(order.borrow().as_slice(), &[1, 2, 3, 1, 2, 3]);
}

#[test]
fn once_subscription_fires_single_time() {
    // 一次性订阅：触发一次后自动取消，后续发布不再调用。
    let mut bus = EventBus::new();
    let calls = Rc::new(RefCell::new(0u32));
    let c = Rc::clone(&calls);
    let _id = bus.subscribe_once(UiEventType::PointerMove, move |_| {
        *c.borrow_mut() += 1;
    });
    bus.publish(&event_of(UiEventType::PointerMove));
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(*calls.borrow(), 1);
    assert_eq!(bus.subscriber_count(), 0);
}

#[test]
fn unsubscribe_is_idempotent() {
    // 注销幂等：重复注销与对无效 id 注销均为 no-op。
    let mut bus = EventBus::new();
    let calls = Rc::new(RefCell::new(0u32));
    let c = Rc::clone(&calls);
    let id = bus.subscribe(UiEventType::PointerMove, move |_| {
        *c.borrow_mut() += 1;
    });
    bus.unsubscribe(id);
    bus.unsubscribe(id);
    bus.unsubscribe(9999);
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(*calls.borrow(), 0);
}

#[test]
fn clear_removes_all_subscribers() {
    // 清空后无处理器被调用，计数归零。
    let mut bus = EventBus::new();
    let calls = Rc::new(RefCell::new(0u32));
    let c1 = Rc::clone(&calls);
    let _a = bus.subscribe(UiEventType::PointerMove, move |_| {
        *c1.borrow_mut() += 1;
    });
    let c2 = Rc::clone(&calls);
    let _b = bus.subscribe_all(move |_| {
        *c2.borrow_mut() += 1;
    });
    bus.clear();
    assert_eq!(bus.subscriber_count(), 0);
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(*calls.borrow(), 0);
}

#[test]
fn publish_without_subscribers_is_noop() {
    // 无订阅者时发布安全返回。
    let bus = EventBus::new();
    bus.publish(&event_of(UiEventType::PointerMove));
    assert_eq!(bus.subscriber_count(), 0);
}
