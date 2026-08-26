//! G-08 验证矩阵：无订阅者、多订阅者、精确类型、注销线性化、
//! 失败隔离、分发期增删、嵌套发布、深度限制、关闭竞争、空壳句柄。

use std::any::TypeId;
use std::cell::Cell;

use super::*;

/// 测试事实：数值负载。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FactA(u32);

/// 测试事实：文本负载（验证精确类型分发）。
#[derive(Debug, Clone, PartialEq, Eq)]
struct FactB(&'static str);

/// 便捷断言：把 Result 转为值，测试中失败直接 panic。
fn ok<T>(r: Result<T>) -> T {
    match r {
        Ok(v) => v,
        Err(e) => panic!("unexpected error: {}", e.short_what()),
    }
}

#[test]
fn publish_without_subscribers_returns_empty_report() {
    // 无订阅者时发布安全返回，不执行任何处理器。
    let bus = EventBus::new();
    let report = ok(bus.publish(FactA(1)));
    assert_eq!(report.matched, 0);
    assert_eq!(report.executed, 0);
    assert_eq!(report.failed, 0);
}

#[test]
fn multiple_subscribers_all_invoked() {
    // 多个订阅者都被调用；测试不依赖调用顺序，只计数。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h1 = Rc::clone(&hits);
    let h2 = Rc::clone(&hits);
    let _s1 = ok(bus.subscribe(move |_: &FactA| h1.set(h1.get() + 1)));
    let _s2 = ok(bus.subscribe(move |_: &FactA| h2.set(h2.get() + 1)));
    let report = ok(bus.publish(FactA(7)));
    assert_eq!(report.matched, 2);
    assert_eq!(report.executed, 2);
    assert_eq!(hits.get(), 2);
}

#[test]
fn dispatch_is_exact_type_only() {
    // 只匹配精确事件类型：订阅 FactA 的处理器收不到 FactB。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h = Rc::clone(&hits);
    let _s = ok(bus.subscribe(move |_: &FactA| h.set(h.get() + 1)));
    let report = ok(bus.publish(FactB("hello")));
    assert_eq!(report.matched, 0);
    assert_eq!(hits.get(), 0);
    ok(bus.publish(FactA(2)));
    assert_eq!(hits.get(), 1);
}

#[test]
fn stable_dispatch_reuses_snapshot_and_mutation_invalidates_exact_type() {
    // 两种事实各注册一个处理器，分别建立类型快照。
    let mut bus = EventBus::new();
    let _a = ok(bus.subscribe(|_: &FactA| {}));
    let _b = ok(bus.subscribe(|_: &FactB| {}));
    ok(bus.publish(FactA(1)));
    ok(bus.publish(FactB("first")));
    let first_a = Rc::clone(
        bus.registry
            .borrow()
            .snapshot_cache
            .get(&TypeId::of::<FactA>())
            .expect("FactA snapshot should be cached"),
    );
    let first_b = Rc::clone(
        bus.registry
            .borrow()
            .snapshot_cache
            .get(&TypeId::of::<FactB>())
            .expect("FactB snapshot should be cached"),
    );

    // 无订阅变更的重复发布必须继续使用同一快照 owner。
    ok(bus.publish(FactA(2)));
    assert!(Rc::ptr_eq(
        &first_a,
        bus.registry
            .borrow()
            .snapshot_cache
            .get(&TypeId::of::<FactA>())
            .expect("stable FactA snapshot should remain cached")
    ));

    // 新增 FactA 只失效 FactA；无关 FactB 快照保持可复用。
    let mut added = ok(bus.subscribe(|_: &FactA| {}));
    {
        let registry = bus.registry.borrow();
        assert!(!registry.snapshot_cache.contains_key(&TypeId::of::<FactA>()));
        assert!(Rc::ptr_eq(
            &first_b,
            registry
                .snapshot_cache
                .get(&TypeId::of::<FactB>())
                .expect("unrelated FactB snapshot should remain cached")
        ));
    }
    ok(bus.publish(FactA(3)));
    let rebuilt_a = Rc::clone(
        bus.registry
            .borrow()
            .snapshot_cache
            .get(&TypeId::of::<FactA>())
            .expect("FactA snapshot should be rebuilt"),
    );
    assert!(!Rc::ptr_eq(&first_a, &rebuilt_a));

    // 注销同样立即失效对应类型，避免后续发布命中过期处理器。
    added.unsubscribe();
    assert!(
        !bus.registry
            .borrow()
            .snapshot_cache
            .contains_key(&TypeId::of::<FactA>())
    );

    // 关闭边界释放所有剩余类型快照。
    ok(bus.close());
    assert!(bus.registry.borrow().snapshot_cache.is_empty());
}

#[test]
fn unsubscribe_prevents_future_delivery_and_is_idempotent() {
    // 注销的线性化点阻止后续发布快照取得处理器；重复释放与
    // 所有权转移不遗留绑定。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h1 = Rc::clone(&hits);
    let h2 = Rc::clone(&hits);
    let mut s1 = ok(bus.subscribe(move |_: &FactA| h1.set(h1.get() + 1)));
    let _s2 = ok(bus.subscribe(move |_: &FactA| h2.set(h2.get() + 1)));
    ok(bus.publish(FactA(1)));
    assert_eq!(hits.get(), 2);
    // 显式注销 + 重复注销（幂等）。
    s1.unsubscribe();
    s1.unsubscribe();
    ok(bus.publish(FactA(2)));
    assert_eq!(hits.get(), 3);
    // 注销后句柄 Drop 仍安全（no-op）。
    drop(s1);
    ok(bus.publish(FactA(3)));
    assert_eq!(hits.get(), 4);
}

#[test]
fn subscription_drop_unsubscribes() {
    // 句柄 Drop 时自动注销（唯一释放责任方）。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h = Rc::clone(&hits);
    let s = ok(bus.subscribe(move |_: &FactA| h.set(h.get() + 1)));
    ok(bus.publish(FactA(1)));
    assert_eq!(hits.get(), 1);
    drop(s);
    ok(bus.publish(FactA(2)));
    assert_eq!(hits.get(), 1);
}

#[test]
fn handler_panic_is_isolated_and_others_still_run() {
    // 处理器 panic 被隔离并计数；其他处理器仍获得执行机会，
    // 已发布事实不被撤销（P-05）。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h = Rc::clone(&hits);
    let _s1 = ok(bus.subscribe(|_: &FactA| panic!("handler 1 故意失败")));
    let _s2 = ok(bus.subscribe(move |_: &FactA| h.set(h.get() + 1)));
    let report = ok(bus.publish(FactA(1)));
    assert_eq!(report.matched, 2);
    assert_eq!(report.executed, 2);
    assert_eq!(report.failed, 1);
    assert_eq!(hits.get(), 1);
    // 失败隔离后总线仍可继续服务。
    ok(bus.publish(FactA(2)));
    assert_eq!(hits.get(), 2);
}

#[test]
fn registration_during_dispatch_takes_effect_next_round() {
    // 分发期间的注册延迟到下一轮生效（快照语义，P-04）。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h1 = Rc::clone(&hits);
    let h2 = Rc::clone(&hits);
    let registered = Rc::new(Cell::new(false));
    let flag = Rc::clone(&registered);
    // 处理器内注册需要注册表句柄（不经 EventBus 借用，避免冲突）。
    let reg_handle = Rc::clone(&bus.registry);
    // 处理器 1：仅首次执行时经注册表句柄注册处理器 2。
    let _s1 = ok(bus.subscribe(move |_: &FactA| {
        h1.set(h1.get() + 1);
        if !flag.get() {
            flag.set(true);
            let h2_inner = Rc::clone(&h2);
            let _ = reg_handle.borrow_mut().subscribe(move |_: &FactA| {
                h2_inner.set(h2_inner.get() + 1);
            });
        }
    }));
    // 第一轮：只有处理器 1 执行（处理器 2 在分发中注册，延迟生效）。
    let report = ok(bus.publish(FactA(1)));
    assert_eq!(report.matched, 1);
    assert_eq!(hits.get(), 1);
    // 第二轮：处理器 1、2 都执行（处理器 1 不再重复注册）。
    let report = ok(bus.publish(FactA(2)));
    assert_eq!(report.matched, 2);
    assert_eq!(hits.get(), 3);
}

#[test]
fn unsubscription_during_dispatch_takes_effect_next_round() {
    // 处理器内注销自己：当前快照已取得，本轮仍执行；
    // 下一轮快照不再取得（注销延迟生效 + 注销线性化语义）。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h_a = Rc::clone(&hits);
    let holder = Rc::new(RefCell::new(None::<Subscription>));
    let holder_a = Rc::clone(&holder);
    // 处理器 A：计数；每次执行时尝试注销自己的句柄（幂等）。
    let s_a = ok(bus.subscribe(move |_: &FactA| {
        h_a.set(h_a.get() + 1);
        holder_a.borrow_mut().take();
    }));
    // 句柄所有权转移给 holder（处理器内 Drop 即自注销）。
    *holder.borrow_mut() = Some(s_a);
    // 第一轮：处理器 A 已进入快照，即使执行中注销也仍执行。
    ok(bus.publish(FactA(1)));
    assert_eq!(hits.get(), 1);
    // 第二轮：注销已生效，处理器 A 不再执行。
    ok(bus.publish(FactA(2)));
    assert_eq!(hits.get(), 1);
}

#[test]
fn nested_publish_uses_publisher_port_in_call_order() {
    // 处理器内嵌套发布经窄发布端口（P-06）：嵌套事件按调用栈
    // 顺序执行（声明顺序）。
    let mut bus = EventBus::new();
    let order = Rc::new(RefCell::new(Vec::new()));
    let outer_order = Rc::clone(&order);
    let inner_order = Rc::clone(&order);
    // 处理器 1 的窄发布端口：仅发布 FactA，不接触完整 Bus。
    let port = bus.publisher::<FactA>();
    // 处理器 1：仅对值 0（外层事实）嵌套发布值 1，避免自递归。
    let _s1 = ok(bus.subscribe(move |f: &FactA| {
        outer_order.borrow_mut().push(format!("outer:{}", f.0));
        if f.0 == 0 {
            let _ = port.publish(FactA(1));
        }
    }));
    // 处理器 2：记录收到的每一条事实。
    let _s2 = ok(bus.subscribe(move |f: &FactA| {
        inner_order.borrow_mut().push(format!("inner:{}", f.0));
    }));
    ok(bus.publish(FactA(0)));
    // 预期顺序：outer:0 →（嵌套）inner:1（outer 处理器占用中跳过）
    // →（返回外层）inner:0。
    assert_eq!(
        order.borrow().as_slice(),
        &[
            "outer:0".to_string(),
            "inner:1".to_string(),
            "inner:0".to_string(),
        ]
    );
}

#[test]
fn reentrant_hit_of_same_handler_is_skipped_and_counted() {
    // 处理器不可重入：执行期间嵌套发布再次命中同一处理器时跳过
    // 并计数（skipped），处理器随后自动放回，不丢失。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h = Rc::clone(&hits);
    let inner_report = Rc::new(RefCell::new(None::<DispatchReport>));
    let store = Rc::clone(&inner_report);
    let port = bus.publisher::<FactA>();
    // 处理器：计数并嵌套发布（嵌套命中自己时被跳过）。
    let _s = ok(bus.subscribe(move |_: &FactA| {
        h.set(h.get() + 1);
        let r = port.publish(FactA(0));
        *store.borrow_mut() = r.ok();
    }));
    let report = ok(bus.publish(FactA(1)));
    // 外层分发：处理器正常执行一次。
    assert_eq!(report.matched, 1);
    assert_eq!(report.executed, 1);
    assert_eq!(report.skipped, 0);
    // 内层分发：命中同一处理器但占用中 → 跳过并计数。
    let inner = match inner_report.borrow().as_ref() {
        Some(r) => *r,
        // 内层分发报告缺失：附带外层报告便于定位时序问题。
        None => panic!("嵌套分发报告缺失 (outer report = {report:?})"),
    };
    assert_eq!(inner.matched, 1);
    assert_eq!(inner.executed, 0);
    assert_eq!(inner.skipped, 1);
    assert_eq!(hits.get(), 1);
    // 处理器已放回：后续发布仍能命中。
    ok(bus.publish(FactA(2)));
    assert_eq!(hits.get(), 2);
}

#[test]
fn depth_limit_rejects_overflow() {
    // 嵌套深度达到上限时发布返回错误，本轮不执行处理器。
    let bus = EventBus::new();
    // 直接置位嵌套深度（测试同模块可访问私有注册表）。
    bus.registry.borrow_mut().dispatch_depth = MAX_DISPATCH_DEPTH;
    let result = bus.publish(FactA(1));
    assert!(result.is_err());
    // 上限内正常放行。
    bus.registry.borrow_mut().dispatch_depth = MAX_DISPATCH_DEPTH - 1;
    let report = ok(bus.publish(FactA(1)));
    assert_eq!(report.matched, 0);
}

#[test]
fn recursive_publish_terminates_safely_under_reentrancy_guard() {
    // 递归发布在处理器不可重入约束下安全终止（不会无限递归），
    // 总线保持可用。
    let mut bus = EventBus::new();
    let depth = Rc::new(Cell::new(0usize));
    let d = Rc::clone(&depth);
    let port = bus.publisher::<FactA>();
    // 处理器：计数并嵌套发布（嵌套命中自己时被跳过）。
    let _s = ok(bus.subscribe(move |_: &FactA| {
        d.set(d.get() + 1);
        let _ = port.publish(FactA(0));
    }));
    let report = ok(bus.publish(FactA(1)));
    // 同一处理器占用中嵌套发布被跳过：仅外层执行一次。
    assert_eq!(report.executed, 1);
    assert_eq!(depth.get(), 1);
    // 总线仍可继续服务。
    ok(bus.publish(FactA(2)));
    assert_eq!(depth.get(), 2);
}

#[test]
fn close_rejects_registration_and_publish() {
    // 关闭后注册与发布失败（状态错误可诊断），注销为幂等 no-op。
    let mut bus = EventBus::new();
    let hits = Rc::new(Cell::new(0u32));
    let h = Rc::clone(&hits);
    let mut s = ok(bus.subscribe(move |_: &FactA| h.set(h.get() + 1)));
    ok(bus.close());
    assert_eq!(bus.state(), BusState::Closed);
    // 关闭后发布失败。
    assert!(bus.publish(FactA(1)).is_err());
    // 关闭后注册失败。
    assert!(bus.subscribe(|_: &FactA| {}).is_err());
    // 关闭后注销为幂等 no-op。
    s.unsubscribe();
    assert_eq!(hits.get(), 0);
}

#[test]
fn close_fails_while_dispatch_in_flight() {
    // 分发在途时关闭返回 WouldBlock；排空后可关闭。
    let mut bus = EventBus::new();
    // 处理器内尝试关闭注册表：此时嵌套分发在途，必须失败。
    let reg_handle = Rc::clone(&bus.registry);
    let _s = ok(bus.subscribe(move |_: &FactA| {
        assert!(reg_handle.borrow_mut().close().is_err());
    }));
    ok(bus.publish(FactA(1)));
    // 分发排空后关闭成功。
    ok(bus.close());
    assert_eq!(bus.state(), BusState::Closed);
}

#[test]
fn publisher_rejects_after_close() {
    // 窄发布端口在 Bus 关闭后发布返回 InvalidState。
    let mut bus = EventBus::new();
    let port = bus.publisher::<FactA>();
    ok(bus.close());
    assert!(port.publish(FactA(1)).is_err());
}

#[test]
fn subscription_outlives_bus_as_inert_shell() {
    // 句柄可以在 Bus 销毁后存在：只能成为失效空壳，释放安全。
    let mut bus = EventBus::new();
    let s = ok(bus.subscribe(|_: &FactA| {}));
    assert!(s.is_active());
    drop(bus);
    // Bus 已销毁：句柄失效，Drop 安全（Weak 升级失败路径）。
    assert!(!s.is_active());
    drop(s);
}

#[test]
fn closed_subscription_is_inactive() {
    // Bus 关闭后句柄变为失效空壳，不继续代表活动绑定。
    let mut bus = EventBus::new();
    let s = ok(bus.subscribe(|_: &FactA| {}));
    assert!(s.is_active());
    ok(bus.close());
    assert!(!s.is_active());
}
