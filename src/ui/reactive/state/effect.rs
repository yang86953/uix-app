// 引入按槽身份管理订阅租约的映射类型。
use std::collections::HashMap;
// 引入并发原子、共享所有权与可恢复读写锁。
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock, Weak};

// 引入依赖收集与稳定槽身份。
use super::{StateSlotId, collect_deps};
// 引入 View 构建期间交接 Effect 的捕获栈。
use super::STATE_CAPTURE_STACK;

// 分配永不回绕的私有订阅令牌。
static NEXT_EFFECT_SUBSCRIBER_TOKEN: AtomicU64 = AtomicU64::new(1);

// 统一 State 与 Computed 下游的最小失效通知契约。
pub(crate) trait DependencySubscriber: Send + Sync {
    // 在源注册表锁外接收一次上游变更。
    fn notify(&self);
}

// 统一 State 与 Computed 向下游暴露的窄依赖源契约。
pub(crate) trait DependencySource: Send + Sync {
    // 读取当前可订阅 generation，调用方不得持有源注册表锁。
    fn generation(&self) -> u64;
    // 借用与值或缓存锁分离的下游注册表。
    fn subscribers(&self) -> &DependencySubscriberRegistry;
}

// 保存一次依赖读取的快照与订阅入口。
pub(crate) struct EffectDependency {
    // 标识本次读取所属的稳定槽。
    pub(crate) slot_id: StateSlotId,
    // 保存读取值时观察到的 generation。
    pub(crate) observed_generation: u64,
    // 复用源自身的 generation 与订阅能力，避免每轮为唯一依赖装箱两个闭包。
    pub(crate) source: Arc<dyn DependencySource>,
}

// 保存依赖源与读取时观察到的 generation。
pub(crate) type GenerationSnapshot = (Arc<dyn DependencySource>, u64);

// 用弱引用保存源不拥有下游生命周期的订阅表。
pub(crate) type DependencySubscribers = HashMap<u64, Weak<dyn DependencySubscriber>>;
// 让值锁与订阅表锁保持完全分离。
pub(crate) type DependencySubscriberRegistry = RwLock<DependencySubscribers>;

// 保存锁外通知所需的最小存活下游快照。
pub(crate) enum DependencySubscriberSnapshot {
    // 没有存活下游时不构造堆容器。
    Empty,
    // 常见单下游直接持有强引用，避免一元 Vec 分配。
    One(Arc<dyn DependencySubscriber>),
    // 多下游继续使用有序堆快照。
    Many(Vec<Arc<dyn DependencySubscriber>>),
}

impl DependencySubscriberSnapshot {
    // 按当前形状追加一个存活下游。
    fn push(&mut self, subscriber: Arc<dyn DependencySubscriber>) {
        match self {
            // 首个下游只提升枚举形状。
            Self::Empty => *self = Self::One(subscriber),
            // 此分支只会用于注册表在快照期间出现防御性增长。
            Self::One(first) => {
                let first = Arc::clone(first);
                *self = Self::Many(vec![first, subscriber]);
            }
            // 已有多下游快照按注册表顺序追加。
            Self::Many(subscribers) => subscribers.push(subscriber),
        }
    }

    // 暴露窄计数供私有生命周期回归使用。
    #[cfg(test)]
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Many(subscribers) => subscribers.len(),
        }
    }
}

// 保存一次订阅的精确注销责任。
pub(crate) struct EffectLease {
    // 强持有依赖源直到精确注销完成，与 generation 快照保持同一生命周期。
    source: Option<Arc<dyn DependencySource>>,
    // 保存本租约独占且永不复用的订阅令牌。
    token: u64,
}

impl EffectLease {
    // 构造由析构负责从同一依赖源精确注销的唯一租约。
    fn new(source: Arc<dyn DependencySource>, token: u64) -> Self {
        // 具体字段即可覆盖 State 与 Computed，无需为释放动作单独分配闭包。
        Self {
            source: Some(source),
            token,
        }
    }
}

impl Drop for EffectLease {
    // 在租约离开最后所有者时精确注销。
    fn drop(&mut self) {
        // 取走源保证重复析构路径幂等。
        let Some(source) = self.source.take() else {
            // 空租约无需继续处理。
            return;
        };
        // 仅短暂获取源注册表写锁并精确删除本令牌。
        source
            .subscribers()
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .remove(&self.token);
    }
}

// 取得永不复用的订阅令牌以避免 ABA。
pub(crate) fn next_subscriber_token() -> u64 {
    // 从当前候选值开始无锁竞争。
    let mut current = NEXT_EFFECT_SUBSCRIBER_TOKEN.load(Ordering::Relaxed);
    // 重试直到独占一个单调令牌。
    loop {
        // 拒绝溢出回绕破坏唯一性。
        assert_ne!(current, u64::MAX, "依赖订阅令牌已耗尽");
        // 计算下一候选值。
        let next = current + 1;
        // 仅在候选未被其他线程占用时提交。
        match NEXT_EFFECT_SUBSCRIBER_TOKEN.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            // 返回本次独占的令牌。
            Ok(token) => return token,
            // 以最新候选继续竞争。
            Err(observed) => current = observed,
        }
    }
}

// 为任意依赖源建立注册后 generation 复核的订阅。
pub(crate) fn subscribe(
    // 接收同时提供 generation 与独立注册表的窄源能力。
    source: Arc<dyn DependencySource>,
    // 接收不被源强持有的下游失效对象。
    subscriber: Arc<dyn DependencySubscriber>,
    // 接收读取值时记录的 generation。
    observed_generation: u64,
) -> EffectLease {
    // 分配不可重用令牌。
    let token = next_subscriber_token();
    // 先注册以覆盖观察与订阅之间的写入。
    {
        // 获取独立注册表写锁并从中毒恢复。
        let mut subscribers = source
            .subscribers()
            .write()
            .unwrap_or_else(|error| error.into_inner());
        // 仅保存弱引用避免源延长下游生命周期。
        subscribers.insert(token, Arc::downgrade(&subscriber));
    }
    // 在注册表锁释放后读取当前 generation。
    let current_generation = source.generation();
    // 补偿读取与订阅之间发生的任何变更。
    if current_generation != observed_generation {
        // 直接在锁外通知下游保留下一轮工作。
        subscriber.notify();
    }
    // 将源与令牌的注销责任交给不可复制租约。
    EffectLease::new(source, token)
}

// 快照并回收存活下游，绝不在注册表锁内通知。
pub(crate) fn collect_subscribers(
    // 接收需要快照的独立注册表。
    registry: &DependencySubscriberRegistry,
) -> DependencySubscriberSnapshot {
    // 获取写锁以同时清理死亡弱引用。
    let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
    // 只有复数登记项时预分配堆快照，单下游保持栈上形状。
    let mut live = if subscribers.len() > 1 {
        DependencySubscriberSnapshot::Many(Vec::with_capacity(subscribers.len()))
    } else {
        DependencySubscriberSnapshot::Empty
    };
    // 保留可升级项并收集其强引用。
    subscribers.retain(|_, subscriber| {
        // 升级弱引用以取得锁外可通知的句柄。
        let Some(subscriber) = subscriber.upgrade() else {
            // 删除已经析构的下游。
            return false;
        };
        // 将通知延后到锁释放后。
        live.push(subscriber);
        // 保留仍活跃的登记项。
        true
    });
    // 返回锁外通知快照。
    live
}

// 在所有源锁外广播同步失效。
pub(crate) fn notify_subscribers(subscribers: DependencySubscriberSnapshot) {
    match subscribers {
        // 空快照没有需要交付的失效。
        DependencySubscriberSnapshot::Empty => {}
        // 单下游直接通知，不经过堆向量。
        DependencySubscriberSnapshot::One(subscriber) => subscriber.notify(),
        // 多下游仍按注册表快照顺序同步交付。
        DependencySubscriberSnapshot::Many(subscribers) => {
            for subscriber in subscribers {
                subscriber.notify();
            }
        }
    }
}

// 以当前依赖差集更新租约与 generation 快照。
pub(crate) fn refresh_dependency_leases(
    // 接收本轮捕获的原始依赖。
    mut deps: Vec<EffectDependency>,
    // 接收下游唯一拥有的租约映射。
    leases: &RwLock<HashMap<StateSlotId, EffectLease>>,
    // 接收本轮要发布的 generation 快照存储。
    snapshots: &RwLock<Vec<GenerationSnapshot>>,
    // 接收订阅时传给每个源的下游失效对象。
    subscriber: Arc<dyn DependencySubscriber>,
) {
    // Effect 的 running 租约与 Computed 的 gate 保证同一 owner 只有一轮刷新。
    // 在短锁内取走已发布容器，后续操作将复用它们的容量。
    let mut current_leases = {
        let mut held = leases.write().unwrap_or_else(|error| error.into_inner());
        std::mem::take(&mut *held)
    };
    let mut next_snapshots = {
        let mut published = snapshots.write().unwrap_or_else(|error| error.into_inner());
        std::mem::take(&mut *published)
    };
    // 在首读顺序下重建快照，向量仅改变长度而不重新分配。
    next_snapshots.clear();
    next_snapshots.extend(
        deps.iter()
            .map(|dependency| (dependency.source.clone(), dependency.observed_generation)),
    );
    // 按稳定槽身份排序，使旧租约差集可以无临时 HashSet 二分查询。
    deps.sort_unstable_by_key(|dependency| dependency.slot_id.0);
    // DependencyCollector 已按槽去重；此处仅在调试构建中锁定该上游契约。
    debug_assert!(
        deps.windows(2)
            .all(|pair| pair[0].slot_id != pair[1].slot_id),
        "依赖捕获必须在刷新前按槽去重"
    );
    // 移除本轮缺失的租约；当前未持有租约锁，Drop 可安全取得源注册表。
    current_leases.retain(|slot, _| {
        deps.binary_search_by_key(&slot.0, |dependency| dependency.slot_id.0)
            .is_ok()
    });
    // 为本轮新出现的源建立租约，全程不持有 owner 锁。
    for dependency in deps {
        if !current_leases.contains_key(&dependency.slot_id) {
            let lease = subscribe(
                dependency.source,
                subscriber.clone(),
                dependency.observed_generation,
            );
            current_leases.insert(dependency.slot_id, lease);
        }
    }
    // 先发布完整租约集，再交接与本轮读取一致的 generation 快照。
    *leases.write().unwrap_or_else(|error| error.into_inner()) = current_leases;
    *snapshots.write().unwrap_or_else(|error| error.into_inner()) = next_snapshots;
}

// 统计存活订阅，供私有生命周期回归使用。
#[cfg(test)]
// 不扩展公开响应式 API。
pub(crate) fn subscriber_count(registry: &DependencySubscriberRegistry) -> usize {
    // 快照函数同时完成死亡弱引用清理。
    collect_subscribers(registry).len()
}

// 保存 Effect 的闭包、快照、租约与并发状态。
struct EffectInner {
    // 用户副作用始终在所有内部锁外调用。
    effect_fn: Arc<dyn Fn() + Send + Sync>,
    // 保存最近成功读取的 generation 快照。
    deps: RwLock<Vec<GenerationSnapshot>>,
    // 保存当前每个源槽唯一的订阅租约。
    leases: RwLock<HashMap<StateSlotId, EffectLease>>,
    // 标记下一轮 tick 是否需要检查。
    pending: AtomicBool,
    // 串行化同一 Effect 的用户闭包执行。
    running: AtomicBool,
}

impl DependencySubscriber for EffectInner {
    // 上游变更仅置 pending，不运行用户闭包。
    fn notify(&self) {
        // 使用释放语义让 tick 观察到本轮失效。
        self.pending.store(true, Ordering::Release);
    }
}

// 在返回或 unwind 时归还 Effect 运行权。
struct RunningLease<'a> {
    // 借用唯一运行标记。
    running: &'a AtomicBool,
}

impl Drop for RunningLease<'_> {
    // 任何路径都释放并发运行权。
    fn drop(&mut self) {
        // 允许下一轮竞争运行权。
        self.running.store(false, Ordering::Release);
    }
}

/// 创建时立即运行，并自动订阅读取到的响应式依赖。
#[derive(Clone)]
// 保持既有可复制公开句柄契约。
pub struct Effect {
    // 所有克隆共享一组租约和 pending 状态。
    inner: Arc<EffectInner>,
}

impl Effect {
    /// 创建副作用，立即执行一次并捕获本轮读取的依赖。
    pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
        // 提升用户闭包为可共享所有权。
        let effect_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(f);
        // 首次调用在锁外收集依赖。
        let (_, deps) = collect_deps(|| effect_fn());
        // 构造尚未安装租约的内部状态。
        let effect = Self {
            // 初始化全部并发与生命周期字段。
            inner: Arc::new(EffectInner {
                // 保存后续 tick 调用的用户闭包。
                effect_fn,
                // 首次刷新将写入此快照。
                deps: RwLock::new(Vec::new()),
                // 首次刷新将写入此租约映射。
                leases: RwLock::new(HashMap::new()),
                // 首次运行后尚无待处理通知。
                pending: AtomicBool::new(false),
                // 初始没有 tick 在执行。
                running: AtomicBool::new(false),
            }),
        };
        // 安装首次捕获的依赖租约。
        effect.refresh_deps(deps);
        // 将构建期 Effect 交接给当前最内层 View 帧。
        STATE_CAPTURE_STACK.with(|stack| {
            // 获取当前捕获栈顶。
            let mut stack = stack.borrow_mut();
            // 非 View 构建期无需交接。
            let Some(output) = stack.last_mut() else {
                // 保持调用方拥有的独立 Effect。
                return;
            };
            // 交接一个共享句柄保持租约生命周期。
            output.effects.push(effect.clone());
        });
        // 返回公开句柄。
        effect
    }

    /// 返回上游依赖是否已请求下一轮执行。
    pub fn has_pending(&self) -> bool {
        // 使用获取语义读取锁外通知。
        self.inner.pending.load(Ordering::Acquire)
    }

    // 用共享差集逻辑刷新本 Effect 的依赖。
    fn refresh_deps(&self, deps: Vec<EffectDependency>) {
        // 转换为通知时不延长公开 Effect 句柄的私有对象。
        let subscriber: Arc<dyn DependencySubscriber> = self.inner.clone();
        // 在所有 Effect 锁外完成订阅与租约析构。
        refresh_dependency_leases(deps, &self.inner.leases, &self.inner.deps, subscriber);
    }

    /// 消费待处理通知，并在依赖确实变化时重新运行副作用。
    ///
    /// 当本次调用实际重新运行了副作用时返回 `true`。
    pub fn tick(&self) -> bool {
        // 只有一个竞争者可消费本轮 pending。
        if self
            .inner
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            // 竞争失败者绝不清除 pending。
            return false;
        }
        // 用 RAII 覆盖用户闭包 panic。
        let _running = RunningLease {
            running: &self.inner.running,
        };
        // 仅拥有运行权者消费 pending。
        if !self.inner.pending.swap(false, Ordering::AcqRel) {
            // 没有通知时无需检查或调用用户代码。
            return false;
        }
        // 在短读锁内比较当前 generation。
        let need_run = self
            .inner
            .deps
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .iter()
            .any(|(source, observed)| source.generation() != *observed);
        // generation 未变仅完成本轮通知消费。
        if !need_run {
            // 不执行用户闭包。
            return false;
        }
        // 在所有锁外重新执行用户闭包并收集新依赖。
        let (_, deps) = collect_deps(|| (self.inner.effect_fn)());
        // 刷新租约时保留运行期间产生的新 pending。
        self.refresh_deps(deps);
        // 报告本轮确实执行了副作用。
        true
    }
}
