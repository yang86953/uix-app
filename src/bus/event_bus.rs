//! EventBus 核心：注册表、同步分发与生命周期状态机。

use std::any::{Any, TypeId};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use crate::core::error::{Errc, Error, Result};

use super::Fact;
use super::subscription::Subscription;

/// 嵌套分发深度上限，防止无界递归发布导致栈溢出。
pub const MAX_DISPATCH_DEPTH: usize = 64;

/// Bus 生命周期状态。
///
/// 同步单线程模型合并了「关闭过程」：无在途分发时 `close` 立即完成；
/// 有在途分发时 `close` 返回 [`Errc::WouldBlock`]，由所有者排空后重试。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BusState {
    /// 正常服务：允许注册与发布。
    #[default]
    Active,
    /// 已关闭：注册与发布返回 `Errc::InvalidState`；订阅句柄变为失效空壳。
    Closed,
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
    /// 按事实类型缓存不可变处理器快照；注册变更只失效对应类型。
    snapshot_cache: HashMap<TypeId, Rc<Vec<HandlerCell>>>,
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
        self.snapshot_cache.remove(&TypeId::of::<F>());
        Ok(id)
    }

    /// 注销单一条目并失效其事实类型快照；重复注销保持 no-op。
    pub(crate) fn unsubscribe(&mut self, id: usize) {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return;
        };
        let type_id = self.entries[index].type_id;
        self.entries.remove(index);
        self.snapshot_cache.remove(&type_id);
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
        self.snapshot_cache.clear();
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
    // 稳定订阅表直接复用类型快照；首次发布或对应类型变更后才重新扫描。
    let type_id = TypeId::of::<F>();
    let snapshot = match reg.snapshot_cache.get(&type_id) {
        Some(snapshot) => Rc::clone(snapshot),
        None => {
            let snapshot = Rc::new(
                reg.entries
                    .iter()
                    .filter(|entry| entry.type_id == type_id)
                    .map(|entry| Rc::clone(&entry.handler))
                    .collect(),
            );
            reg.snapshot_cache.insert(type_id, Rc::clone(&snapshot));
            snapshot
        }
    };
    let mut report = DispatchReport {
        matched: snapshot.len(),
        ..DispatchReport::default()
    };
    drop(reg);
    // 逐处理器执行：「取出-调用-放回」，panic 被隔离并计数，
    // 不阻止其他处理器（P-05）；同处理器重入（执行期间再次命中）
    // 跳过并计数，保证语义可诊断。
    for cell in snapshot.iter() {
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
