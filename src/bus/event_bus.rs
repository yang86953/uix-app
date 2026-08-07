//! EventBus 核心：注册表、同步分发与生命周期状态机。

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::marker::PhantomData;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::rc::Rc;

use crate::core::error::{Errc, Error, Result};

use super::subscription::Subscription;
use super::Fact;

/// 嵌套分发深度上限，防止无界递归发布导致栈溢出。
pub const MAX_DISPATCH_DEPTH: usize = 64;

/// Bus 生命周期状态。
///
/// 同步单线程模型合并了「关闭过程」：无在途分发时 `close` 立即完成；
/// 有在途分发时 `close` 返回 [`Errc::WouldBlock`]，由所有者排空后重试。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BusState {
    /// 正常服务：允许注册与发布。
    Active,
    /// 已关闭：注册与发布返回 `Errc::InvalidState`；订阅句柄变为失效空壳。
    Closed,
}

impl Default for BusState {
    fn default() -> Self {
        Self::Active
    }
}

/// 发布技术报告：仅用于诊断分发健康度，不充当业务应答。
///
/// 事实是否成立、某项业务是否被接受，不取决于本报告。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DispatchReport {
    /// 本轮匹配的处理器数量。
    pub matched: usize,
    /// 实际执行的处理器数量。
    pub executed: usize,
    /// 失败（panic 被隔离）的处理器数量。
    pub failed: usize,
    /// 因重入占用未执行的处理器数量（同一处理器执行期间被嵌套
    /// 发布再次命中；处理器不可重入，见 [`EventBus`] 分发语义）。
    pub skipped: usize,
}

/// 处理器存储：共享句柄使发布期快照无需持有注册表排他锁。
///
/// `Cell<Option<..>>` 支持「取出-调用-放回」协议：处理器执行期间
/// 其插槽为空，嵌套发布再次命中时跳过（不可重入），执行完成或
/// panic 后放回，处理器不会丢失。
type HandlerCell = Rc<Cell<Option<Box<dyn FnMut(&dyn Any)>>>>;

/// 注册表条目：事件精确类型 + 处理器共享句柄。
pub(crate) struct Entry {
    pub(crate) id: usize,
    pub(crate) type_id: TypeId,
    pub(crate) handler: HandlerCell,
}

/// 注册表：条目列表、id 分配、生命周期状态与嵌套分发深度。
///
/// 字段与注册 / 关闭方法对 crate 内可见（`Subscription` 注销、窄发布
/// 端口与测试需要），不构成公开契约。
#[derive(Default)]
pub(crate) struct Registry {
    pub(crate) entries: Vec<Entry>,
    pub(crate) next_id: usize,
    pub(crate) state: BusState,
    /// 当前嵌套分发深度（0 = 无分发在途）。
    pub(crate) dispatch_depth: usize,
}

impl Registry {
    /// 注册类型化处理器（crate 内部）：返回条目 id。
    ///
    /// 仅 Active 状态允许注册；关闭后拒绝并返回可诊断错误。
    pub(crate) fn subscribe<F: Fact>(
        &mut self,
        mut handler: impl FnMut(&F) + 'static,
    ) -> Result<usize> {
        if self.state != BusState::Active {
            return Err(Error::invalid_state("EventBus 未处于 Active，拒绝注册"));
        }
        let id = self.next_id;
        self.next_id += 1;
        // 把类型化处理器适配为 Any 分发：快照按 TypeId 精确收集，
        // 类型不匹配在实现上不可能发生，故用 if-let 静默跳过。
        let raw: Box<dyn FnMut(&dyn Any)> = Box::new(move |any: &dyn Any| {
            if let Some(fact) = any.downcast_ref::<F>() {
                handler(fact);
            }
        });
        self.entries.push(Entry {
            id,
            type_id: TypeId::of::<F>(),
            handler: Rc::new(Cell::new(Some(raw))),
        });
        Ok(id)
    }

    /// 关闭注册表（crate 内部）：停止注册与发布，清空条目。
    ///
    /// 有在途分发（嵌套 publish 未返回）时返回 [`Errc::WouldBlock`]，
    /// 由所有者排空后重试。关闭本身幂等。
    pub(crate) fn close(&mut self) -> Result<()> {
        if self.dispatch_depth > 0 {
            return Err(Error::new(
                Errc::WouldBlock,
                "分发在途，不能关闭 EventBus；请排空后重试",
            ));
        }
        self.state = BusState::Closed;
        self.entries.clear();
        Ok(())
    }
}

/// 共享分发逻辑：状态与深度检查、取处理器快照、逐处理器执行、恢复深度。
///
/// 处理器执行前先取匹配快照（浅拷贝 handler 共享句柄），随后释放注册表
/// 借用，使处理器内注册 / 注销 / 嵌套发布不与注册表借用冲突；panic 被
/// `catch_unwind` 隔离并计数（失败隔离并继续，P-05），恢复深度必然可达。
fn dispatch<F: Fact>(registry: &Rc<RefCell<Registry>>, fact: F) -> Result<DispatchReport> {
    let mut reg = registry.borrow_mut();
    // 仅 Active 状态允许发布；关闭后拒绝并返回可诊断错误。
    if reg.state != BusState::Active {
        return Err(Error::invalid_state("EventBus 未处于 Active，拒绝发布"));
    }
    // 嵌套分发深度检查：超限拒绝本轮，防止无界递归。
    if reg.dispatch_depth >= MAX_DISPATCH_DEPTH {
        return Err(Error::new(
            Errc::InvalidState,
            "嵌套分发深度超过上限，拒绝发布",
        ));
    }
    reg.dispatch_depth += 1;
    // 取匹配处理器快照，随后释放注册表借用（处理器执行期无注册表锁）。
    let snapshot: Vec<HandlerCell> = reg
        .entries
        .iter()
        .filter(|e| e.type_id == TypeId::of::<F>())
        .map(|e| Rc::clone(&e.handler))
        .collect();
    let mut report = DispatchReport {
        matched: snapshot.len(),
        ..DispatchReport::default()
    };
    drop(reg);
    // 逐处理器执行：「取出-调用-放回」，panic 被隔离并计数，
    // 不阻止其他处理器（P-05）；同处理器重入（执行期间再次命中）
    // 跳过并计数，保证语义可诊断。
    for cell in &snapshot {
        let mut taken = cell.take();
        match taken.as_mut() {
            Some(h) => {
                report.executed += 1;
                let outcome = catch_unwind(AssertUnwindSafe(|| {
                    h(&fact as &dyn Any);
                }));
                if outcome.is_err() {
                    report.failed += 1;
                }
            }
            None => {
                // 处理器不可重入：占用中跳过，处理器稍后自动放回。
                report.skipped += 1;
            }
        }
        // 无论成败都放回，处理器不会因 panic 丢失。
        cell.set(taken);
    }
    registry.borrow_mut().dispatch_depth -= 1;
    Ok(report)
}

/// 窄发布端口：只允许发布单一事件类型（P-06）。
///
/// 普通业务 Component 只注入本端口，不取得完整 Bus、注册表、订阅表或
/// 销毁权。处理器内嵌套发布应使用本端口（而非重新借用 Bus）。
#[derive(Clone)]
pub struct Publisher<F: Fact> {
    registry: Rc<RefCell<Registry>>,
    _marker: PhantomData<fn(F)>,
}

impl<F: Fact> Publisher<F> {
    /// 发布单一类型事实：语义与 [`EventBus::publish`] 相同。
    pub fn publish(&self, fact: F) -> Result<DispatchReport> {
        dispatch(&self.registry, fact)
    }
}

/// 进程内同步类型化事件分发总线（单线程，`!Send + !Sync`）。
///
/// # 生命周期
///
/// ```text
/// 启动：new → 订阅方注册并保存句柄 → publish
/// 关闭：close（要求无嵌套分发在途）→ 订阅句柄变为失效空壳
/// ```
///
/// 关闭后 `subscribe` / `publish` 返回 `Errc::InvalidState`；关闭期间的
/// 注销行为定义为幂等 no-op（注册表已清空）。
///
/// # 分发语义
///
/// - 处理器执行前先取匹配快照，不在注册表排他保护区内执行用户处理器；
/// - 分发期间的注册 / 注销延迟到下一轮生效；处理器内注销自己在当前
///   快照中已取得的回调仍会执行（与「注销线性化点阻止后续快照取得
///   处理器」一致）；
/// - 处理器内嵌套发布允许（调用栈顺序），深度上限 [`MAX_DISPATCH_DEPTH`]；
///   处理器应使用 [`Publisher`] 窄端口发布，而非重新借用 Bus；
///   **同一处理器不可重入**：执行期间嵌套发布再次命中时跳过并计入
///   `DispatchReport::skipped`；
/// - 处理器 panic 被隔离并计数（失败隔离并继续，P-05），不阻止其他
///   处理器执行，也不撤销已发布事实。
pub struct EventBus {
    registry: Rc<RefCell<Registry>>,
}

impl EventBus {
    /// 创建新总线（初始状态 [`BusState::Active`]）。
    pub fn new() -> Self {
        Self {
            registry: Rc::new(RefCell::new(Registry::default())),
        }
    }

    /// 当前生命周期状态。
    pub fn state(&self) -> BusState {
        self.registry.borrow().state
    }

    /// 按事件契约精确类型注册处理器，返回作用域订阅句柄。
    ///
    /// 句柄由订阅方持有；显式 [`Subscription::unsubscribe`] 或 Drop 时
    /// 自动注销（幂等）。Bus 关闭或销毁后句柄变为失效空壳，释放安全。
    pub fn subscribe<F: Fact>(
        &mut self,
        handler: impl FnMut(&F) + 'static,
    ) -> Result<Subscription> {
        let id = self.registry.borrow_mut().subscribe(handler)?;
        Ok(Subscription {
            registry: Rc::downgrade(&self.registry),
            id,
        })
    }

    /// 取得单一事件类型的窄发布端口（P-06）。
    ///
    /// 端口可克隆并注入普通业务 Component；Bus 关闭后端口发布返回
    /// `Errc::InvalidState`。
    pub fn publisher<F: Fact>(&self) -> Publisher<F> {
        Publisher {
            registry: Rc::clone(&self.registry),
            _marker: PhantomData,
        }
    }

    /// 同步发布事实：返回前本轮匹配处理器已全部执行完毕。
    ///
    /// 返回技术分发报告（仅诊断用）。发布失败（状态错误或嵌套深度超限）
    /// 以 `Err` 返回，本轮不执行任何处理器。
    pub fn publish<F: Fact>(&self, fact: F) -> Result<DispatchReport> {
        dispatch(&self.registry, fact)
    }

    /// 关闭总线：停止新注册与新发布，使已有订阅句柄失效。
    ///
    /// 同步模型中「在途回调」只存在于嵌套分发调用栈内；调用方必须先
    /// 排空（所有嵌套 `publish` 已返回），否则返回 [`Errc::WouldBlock`]，
    /// 由所有者稍后重试。关闭操作本身幂等。
    pub fn close(&mut self) -> Result<()> {
        self.registry.borrow_mut().close()
    }

    /// 当前注册的订阅者数量（诊断用）。
    pub fn subscriber_count(&self) -> usize {
        self.registry.borrow().entries.len()
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    //! G-08 验证矩阵：无订阅者、多订阅者、精确类型、注销线性化、
    //! 失败隔离、分发期增删、嵌套发布、深度限制、关闭竞争、空壳句柄。

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
            None => panic!("嵌套分发报告缺失"),
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
}
