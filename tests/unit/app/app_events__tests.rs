//! 契约与链路验证：精确类型分发、事实负载快照、订阅注销语义。

use std::cell::Cell;
use std::rc::Rc;

use super::*;
use crate::bus::Subscription;

/// 便捷断言：把 Result 转为值，测试中失败直接 panic。
fn ok<T>(r: crate::core::error::Result<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("unexpected error: {}", e.short_what()),
    }
}

#[test]
fn theme_applied_dispatches_to_subscriber() {
    // 主题事实总线：订阅者收到 ThemeApplied 精确类型负载。
    let mut bus = ThemeEventBus::new();
    let seen = Rc::new(Cell::new(None::<ThemeApplied>));
    let s = Rc::clone(&seen);
    let _sub = ok(bus.subscribe(move |fact: &ThemeApplied| {
        s.set(Some(*fact));
    }));
    ok(bus.publish(ThemeApplied {
        is_dark: true,
        revision: 1,
    }));
    assert_eq!(
        seen.get(),
        Some(ThemeApplied {
            is_dark: true,
            revision: 1
        })
    );
}

#[test]
fn narrow_publisher_injects_single_fact_capability() {
    // 窄发布端口（P-06）：只发布 ThemeApplied，不暴露完整 Bus。
    let mut bus = ThemeEventBus::new();
    let port: ThemeEventPublisher = bus.publisher();
    let seen = Rc::new(Cell::new(0u64));
    let s = Rc::clone(&seen);
    let _sub = ok(bus.subscribe(move |fact: &ThemeApplied| {
        s.set(fact.revision);
    }));
    ok(port.publish(ThemeApplied {
        is_dark: false,
        revision: 7,
    }));
    assert_eq!(seen.get(), 7);
}

#[test]
fn subscription_scope_handle_unsubscribes_on_drop() {
    // 订阅句柄 Drop 即注销：后续发布不再投递。
    let mut bus = ThemeEventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h = Rc::clone(&hits);
    let sub: Subscription = ok(bus.subscribe(move |_: &ThemeApplied| {
        h.set(h.get() + 1);
    }));
    ok(bus.publish(ThemeApplied {
        is_dark: true,
        revision: 1,
    }));
    assert_eq!(hits.get(), 1);
    drop(sub);
    ok(bus.publish(ThemeApplied {
        is_dark: true,
        revision: 2,
    }));
    assert_eq!(hits.get(), 1);
}
