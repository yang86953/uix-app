// 引入按槽身份管理订阅租约的映射类型。
use std::collections::{HashMap, HashSet};
// 引入并发原子、共享所有权与可恢复读写锁。
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock, Weak};

// 引入依赖收集、generation 快照与 State 私有内部状态。
use super::{GenerationCheck, GenerationSnapshot, StateInner, StateSlotId, collect_deps};
// 引入 View 构建期间交接 Effect 的捕获栈。
use super::STATE_CAPTURE_STACK;

// 分配永不回绕的私有订阅令牌。
static NEXT_EFFECT_SUBSCRIBER_TOKEN: AtomicU64 = AtomicU64::new(1);

// 统一 State 与 Computed 下游的最小失效通知契约。
pub(crate) trait DependencySubscriber: Send + Sync {
    // 在源注册表锁外接收一次上游变更。
    fn notify(&self);
}

// 保存一次依赖读取的快照与订阅入口。
pub(crate) struct EffectDependency {
    // 标识本次读取所属的稳定槽。
    pub(crate) slot_id: StateSlotId,
    // 保存读取值时观察到的 generation。
    pub(crate) observed_generation: u64,
    // 供 tick 或重算复核当前 generation。
    pub(crate) check_generation: GenerationCheck,
    // 注册下游并返回唯一释放租约。
    pub(crate) subscribe_pending:
        Box<dyn Fn(Arc<dyn DependencySubscriber>, u64) -> EffectLease + Send + Sync>,
}

// 用弱引用保存源不拥有下游生命周期的订阅表。
pub(crate) type DependencySubscribers = HashMap<u64, Weak<dyn DependencySubscriber>>;
// 让值锁与订阅表锁保持完全分离。
pub(crate) type DependencySubscriberRegistry = Arc<RwLock<DependencySubscribers>>;

// 保存一次订阅的精确注销责任。
pub(crate) struct EffectLease {
    // 仅允许首次析构时取走注销动作。
    release: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl EffectLease {
    // 构造由析构负责执行的唯一注销租约。
    pub(crate) fn new<F: FnOnce() + Send + Sync + 'static>(release: F) -> Self {
        // 保存调用方提供的一次性动作。
        Self {
            // 装箱使不同注销闭包拥有统一存储。
            release: Some(Box::new(release)),
        }
    }
}

impl Drop for EffectLease {
    // 在租约离开最后所有者时精确注销。
    fn drop(&mut self) {
        // 取走动作保证重复析构路径幂等。
        let Some(release) = self.release.take() else {
            // 空租约无需继续处理。
            return;
        };
        // 在所有外部锁之外执行注销闭包。
        release();
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
pub(crate) fn subscribe<T>(
    // 接收与值锁分离的源订阅表。
    registry: DependencySubscriberRegistry,
    // 接收可读取 generation 的源内部状态。
    inner: Arc<RwLock<StateInner<T>>>,
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
        let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
        // 仅保存弱引用避免源延长下游生命周期。
        subscribers.insert(token, Arc::downgrade(&subscriber));
    }
    // 在注册表锁释放后读取当前 generation。
    let current_generation = inner
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .generation;
    // 补偿读取与订阅之间发生的任何变更。
    if current_generation != observed_generation {
        // 直接在锁外通知下游保留下一轮工作。
        subscriber.notify();
    }
    // 将令牌注销责任交给不可复制租约。
    EffectLease::new(move || {
        // 仅短暂获取注册表写锁。
        let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
        // 删除本令牌绝不影响后续令牌。
        subscribers.remove(&token);
    })
}

// 快照并回收存活下游，绝不在注册表锁内通知。
pub(crate) fn collect_subscribers(
    // 接收需要快照的独立注册表。
    registry: &DependencySubscriberRegistry,
) -> Vec<Arc<dyn DependencySubscriber>> {
    // 获取写锁以同时清理死亡弱引用。
    let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
    // 为所有存活下游预分配快照空间。
    let mut live = Vec::with_capacity(subscribers.len());
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
pub(crate) fn notify_subscribers(subscribers: Vec<Arc<dyn DependencySubscriber>>) {
    // 逐项调用无用户闭包的私有通知契约。
    for subscriber in subscribers {
        // 让下游负责仅置脏和继续传播。
        subscriber.notify();
    }
}

// 以当前依赖差集更新租约与 generation 快照。
pub(crate) fn refresh_dependency_leases(
    // 接收本轮捕获的原始依赖。
    deps: Vec<EffectDependency>,
    // 接收下游唯一拥有的租约映射。
    leases: &RwLock<HashMap<StateSlotId, EffectLease>>,
    // 接收本轮要发布的 generation 快照存储。
    snapshots: &RwLock<Vec<GenerationSnapshot>>,
    // 接收订阅时传给每个源的下游失效对象。
    subscriber: Arc<dyn DependencySubscriber>,
) {
    // 按槽去重保留首个观察快照。
    let mut next = HashMap::with_capacity(deps.len());
    // 逐项整理本轮依赖。
    for dependency in deps {
        // 同槽重复读取只保留一份租约。
        next.entry(dependency.slot_id).or_insert(dependency);
    }
    // 在短锁内移出不再需要的租约并快照已存在槽。
    let (stale, existing) = {
        // 从中毒恢复并独占租约映射。
        let mut held = leases.write().unwrap_or_else(|error| error.into_inner());
        // 找到当前轮缺失的旧槽。
        let stale_slots: Vec<_> = held
            .keys()
            .copied()
            .filter(|slot| !next.contains_key(slot))
            .collect();
        // 将旧租约移出锁区以便锁外析构。
        let stale: Vec<_> = stale_slots
            .into_iter()
            .filter_map(|slot| held.remove(&slot))
            .collect();
        // 快照本轮可复用的租约槽。
        let existing = held.keys().copied().collect::<HashSet<_>>();
        // 同时交出锁外析构列表和可复用集合。
        (stale, existing)
    };
    // 确保注销闭包不在租约锁内运行。
    drop(stale);
    // 暂存需要安装的新租约。
    let mut additions = HashMap::new();
    // 暂存与本轮读取严格对应的快照。
    let mut next_snapshots = Vec::with_capacity(next.len());
    // 为每个去重依赖建立或复用租约。
    for (slot, dependency) in next {
        // 仅为当前未持有的槽调用源订阅入口。
        if !existing.contains(&slot) {
            // 所有订阅与可能的注册后通知均在租约锁外。
            let lease =
                (dependency.subscribe_pending)(subscriber.clone(), dependency.observed_generation);
            // 暂存至短锁插入阶段。
            additions.insert(slot, lease);
        }
        // 保存读取时 generation 而非延迟读取值。
        next_snapshots.push((dependency.check_generation, dependency.observed_generation));
    }
    // 将新租约插入并移出防御性替换值。
    let replaced = {
        // 仅短暂获取租约写锁。
        let mut held = leases.write().unwrap_or_else(|error| error.into_inner());
        // 不在锁中析构被替换的租约。
        additions
            .into_iter()
            .filter_map(|(slot, lease)| held.insert(slot, lease))
            .collect::<Vec<_>>()
    };
    // 在锁外释放所有意外替换项。
    drop(replaced);
    // 发布与本轮订阅集合一致的 generation 快照。
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

// 创建时立即运行并自动订阅读取到的依赖。
#[derive(Clone)]
// 保持既有可复制公开句柄契约。
pub struct Effect {
    // 所有克隆共享一组租约和 pending 状态。
    inner: Arc<EffectInner>,
}

impl Effect {
    // 创建并执行一次用户副作用。
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

    // 查询是否已有上游请求下一轮 tick。
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

    // 检查失效依赖并在需要时重跑副作用。
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
            .any(|(check, observed)| check() != *observed);
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
