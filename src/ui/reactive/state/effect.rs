// 引入按槽身份去重与持有订阅租约的映射容器。
use std::collections::{HashMap, HashSet};
// 引入副作用实例的共享内部所有权。
use std::sync::{Arc, RwLock, Weak};
// 引入并发 pending、running 标志的原子语义。
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
// 引入依赖快照与租约私有类型。
use super::{collect_deps, GenerationCheck, GenerationSnapshot, StateInner, StateSlotId};
// 引入 View 构建期副作用交接栈。
use super::STATE_CAPTURE_STACK;

// 为 State 内部 Effect 订阅生成全局单调且不复用的令牌。
static NEXT_EFFECT_SUBSCRIBER_TOKEN: AtomicU64 = AtomicU64::new(1);

// 保存 State→Effect 自动订阅所需的读取快照与注册入口。
pub(crate) struct EffectDependency {
    // 标识本次读取所属的稳定 State 槽。
    pub(crate) slot_id: StateSlotId,
    // 保存与读取值同锁取得的 generation。
    pub(crate) observed_generation: u64,
    // 在 tick 检查时读取当前 generation。
    pub(crate) check_generation: GenerationCheck,
    // 建立直接 State pending 订阅并返回唯一租约。
    pub(crate) subscribe_pending: Box<dyn Fn(Arc<AtomicBool>, u64) -> EffectLease + Send + Sync>,
}

// 保存 State 内部不延长 Effect 生命周期的订阅弱引用表。
pub(crate) type EffectSubscribers = HashMap<u64, Weak<AtomicBool>>;
// 保存与 State 值锁隔离的共享订阅注册表。
pub(crate) type EffectSubscriberRegistry = Arc<RwLock<EffectSubscribers>>;

// 保存一次 Effect 订阅的唯一释放责任，禁止租约复制。
pub(crate) struct EffectLease {
    // 保存首次释放时移出的私有注销动作。
    release: Option<Box<dyn FnOnce() + Send + Sync>>,
}

impl EffectLease {
    // 创建一个不需要注销外部资源的空租约。
    pub(crate) fn noop() -> Self {
        // 返回没有注销动作的独立租约。
        Self { release: None }
    }

    // 创建一个由租约析构时执行的私有注销动作。
    pub(crate) fn new<F: FnOnce() + Send + Sync + 'static>(release: F) -> Self {
        // 保存唯一的注销责任。
        Self {
            // 将调用方提供的动作装箱为一次性闭包。
            release: Some(Box::new(release)),
        }
    }
}

impl Drop for EffectLease {
    // 在租约离开当前所有者时立即执行一次注销。
    fn drop(&mut self) {
        // 取走动作使重复析构路径保持幂等。
        let Some(release) = self.release.take() else {
            // 空租约或已释放租约无需继续处理。
            return;
        };
        // 在线程安全的状态注册表中精确移除本订阅。
        release();
    }
}

// 分配永不回绕复用的 Effect 订阅令牌，耗尽时明确终止而非误删新订阅。
pub(crate) fn next_subscriber_token() -> u64 {
    // 从当前候选令牌开始进行无锁单调分配。
    let mut current = NEXT_EFFECT_SUBSCRIBER_TOKEN.load(Ordering::Relaxed);
    loop {
        // 最大值之后将回绕，必须拒绝破坏令牌唯一性。
        assert_ne!(current, u64::MAX, "Effect 订阅令牌已耗尽");
        // 计算唯一且不回绕的后继令牌。
        let next = current + 1;
        // 仅在候选未被其他线程抢占时提交本次分配。
        match NEXT_EFFECT_SUBSCRIBER_TOKEN.compare_exchange_weak(
            current,
            next,
            Ordering::Relaxed,
            Ordering::Relaxed,
        ) {
            // 成功时当前值专属本租约且之后永不复用。
            Ok(token) => return token,
            // 竞争失败时从最新候选重试。
            Err(observed) => current = observed,
        }
    }
}

// 为 Effect 建立可确定释放的弱引用订阅，并在注册后复核 generation。
pub(crate) fn subscribe<T>(
    // 接收与 State 值锁隔离的 Effect 订阅注册表。
    registry: EffectSubscriberRegistry,
    // 接收 State 唯一拥有的内部可变状态。
    inner: Arc<RwLock<StateInner<T>>>,
    // 接收 Effect 持有的 pending 原子标志。
    pending: Arc<AtomicBool>,
    // 接收读取值时观察到的 generation。
    observed_generation: u64,
) -> EffectLease {
    // 分配永不复用的订阅令牌，使旧租约不能误删新订阅。
    let token = next_subscriber_token();
    // 先在独立注册表中建立弱引用，使后续值写入不会遗漏本订阅。
    {
        // 独占访问与 State 值锁无嵌套关系的 Effect 注册表。
        let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
        // 保存弱引用，避免 State 注册表延长 Effect 生命周期。
        subscribers.insert(token, Arc::downgrade(&pending));
    }
    // 注册表锁已释放后才读取 State generation，禁止两把锁嵌套。
    let current_generation = inner
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .generation;
    // 补偿读取与注册之间发生的任何状态变更。
    if current_generation != observed_generation {
        // 保留下一次 Effect tick 所需的 pending 信号。
        pending.store(true, Ordering::Release);
    }
    // 由不可复制租约唯一持有精确注销责任。
    EffectLease::new(move || {
        // 仅短暂取得独立注册表锁以移除属于本租约的令牌。
        let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
        // 删除令牌即使重复析构也保持安全。
        subscribers.remove(&token);
    })
}

// 在独立注册表锁内投影活跃 Effect，并顺带回收已死亡的弱引用。
pub(crate) fn collect_pendings(registry: &EffectSubscriberRegistry) -> Vec<Arc<AtomicBool>> {
    // 独占注册表以便清理死亡弱引用。
    let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
    // 保存可在 State 锁外通知的 Effect pending 标志。
    let mut pendings = Vec::with_capacity(subscribers.len());
    // 保留仍可升级的订阅，同时清理已经析构的 Effect。
    subscribers.retain(|_, pending| {
        // 若 Effect 仍存活则取出一份安全共享引用。
        let Some(pending) = pending.upgrade() else {
            // 死亡 Effect 不应继续占据 State 私有注册表。
            return false;
        };
        // 将通知动作延迟到离开 State 锁之后。
        pendings.push(pending);
        // 保留该仍存活的弱引用。
        true
    });
    // 交给调用者在锁外设置 pending。
    pendings
}

// 在所有 State 锁外设置活跃 Effect 的下一轮 pending 信号。
pub(crate) fn notify_pendings(pendings: Vec<Arc<AtomicBool>>) {
    // 逐个写入原子标志，不执行任何用户闭包。
    for pending in pendings {
        // 保证 Effect tick 能观察到本次 State 变更。
        pending.store(true, Ordering::Release);
    }
}

// 统计仍有效的 State 内部 Effect 订阅，供模块私有测试验证生命周期。
#[cfg(test)]
// 此计数器不构成公开 State 契约。
pub(crate) fn subscriber_count(registry: &EffectSubscriberRegistry) -> usize {
    // 取得独立注册表写锁以同时清理死亡弱引用并读取精确数量。
    let mut subscribers = registry.write().unwrap_or_else(|error| error.into_inner());
    // 移除无需等待下一次 State 写入的已死亡订阅。
    subscribers.retain(|_, pending| pending.strong_count() != 0);
    // 返回仍有效的 Effect 内部订阅数量。
    subscribers.len()
}

// 保存一个 Effect 的闭包、依赖快照和唯一订阅租约所有权。
struct EffectInner {
    // 保存每轮在锁外执行的用户副作用闭包。
    effect_fn: Arc<dyn Fn() + Send + Sync>,
    // 保存最近成功读取对应的 generation 快照。
    deps: std::sync::RwLock<Vec<GenerationSnapshot>>,
    // 以 State 槽身份保存当前唯一的订阅租约。
    leases: std::sync::RwLock<HashMap<StateSlotId, EffectLease>>,
    // 标记下一轮 tick 是否需要检查依赖。
    pending: Arc<AtomicBool>,
    // 串行化同一 Effect 的 tick，避免并行执行用户闭包。
    running: AtomicBool,
}

// 在 tick 返回或 unwind 时归还本轮唯一的运行权。
struct RunningLease<'a> {
    // 借用所属 Effect 的运行标志。
    running: &'a AtomicBool,
}

impl Drop for RunningLease<'_> {
    // 无论用户闭包正常返回或 panic，都释放运行权。
    fn drop(&mut self) {
        // 允许后续 tick 竞争下一轮运行权。
        self.running.store(false, Ordering::Release);
    }
}

/// 创建时执行闭包，自动追踪其中读取的所有 State。
/// 当直接 State 依赖变更时，`tick()` 重新执行；Computed pending 暂不支持。
#[derive(Clone)]
// 保持公开 Effect 句柄的既有可复制兼容契约。
pub struct Effect {
    // 共享内部状态使 View 捕获交接与调用方临时句柄可共存。
    inner: Arc<EffectInner>,
}

impl Effect {
    // 创建并立即执行一次依赖捕获的副作用。
    pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
        // 将用户闭包提升为可供后续 tick 共享调用的所有权。
        let effect_fn: Arc<dyn Fn() + Send + Sync> = Arc::new(f);
        // 首次执行用户闭包以收集初始直接 State 依赖。
        let (_, deps) = collect_deps(|| effect_fn());
        // 创建唯一拥有订阅表与并发标志的副作用内部状态。
        let effect = Self {
            // 初始化尚未安装租约的 Effect 内部状态。
            inner: Arc::new(EffectInner {
                // 保存用户闭包供每次 tick 的锁外调用。
                effect_fn,
                // 保存首次捕获后将写入的依赖快照。
                deps: std::sync::RwLock::new(Vec::new()),
                // 保存由本 Effect 唯一拥有的 State 订阅租约。
                leases: std::sync::RwLock::new(HashMap::new()),
                // 初始执行已经完成，尚无待处理 State 变更。
                pending: Arc::new(AtomicBool::new(false)),
                // 初始没有 tick 正在运行。
                running: AtomicBool::new(false),
            }),
        };
        // 按首次读取快照去重并建立直接 State 订阅。
        effect.refresh_deps(deps);
        // 仅把一个内部共享句柄交给当前最内层 View 构建帧。
        STATE_CAPTURE_STACK.with(|stack| {
            // 取得栈顶输出以保证嵌套构建彼此隔离。
            let mut stack = stack.borrow_mut();
            // 非 View 构建期创建的 Effect 无需交接给树。
            let Some(output) = stack.last_mut() else {
                // 没有活动捕获时保持调用方持有的独立 Effect。
                return;
            };
            // 交接公开兼容的共享句柄，由最后一个内部引用负责释放租约。
            output.effects.push(effect.clone());
        });
        // 将调用方句柄返回，并保持既有 Clone 兼容性。
        effect
    }

    // 查询是否有直接 State 通知要求下一次 tick 检查依赖。
    pub fn has_pending(&self) -> bool {
        // 读取由 State 锁外写入的 pending 信号。
        self.inner.pending.load(Ordering::Acquire)
    }

    // 按本轮读取结果去重、复用仍有效租约并在全部锁外管理租约生命周期。
    fn refresh_deps(&self, deps: Vec<EffectDependency>) {
        // 以槽身份保留每个 State 本轮的第一份依赖快照。
        let mut next_deps = HashMap::with_capacity(deps.len());
        // 同槽重复读取只保留一个订阅与一个 generation 检查器。
        for dep in deps {
            // 第一次读取决定该槽的订阅快照与检查器。
            next_deps.entry(dep.slot_id).or_insert(dep);
        }
        // 在短租约锁内移出旧项并快照仍然有效的槽身份。
        let (stale_leases, existing_slots) = {
            // 独占当前 Effect 的租约映射，但不在锁内析构或订阅。
            let mut leases = self
                .inner
                .leases
                .write()
                .unwrap_or_else(|error| error.into_inner());
            // 收集本轮已不读取的槽身份以便精确移出其租约。
            let stale_slots: Vec<_> = leases
                .keys()
                .copied()
                .filter(|slot_id| !next_deps.contains_key(slot_id))
                .collect();
            // 将旧租约移出锁区，析构与注销将在锁外发生。
            let stale_leases: Vec<_> = stale_slots
                .into_iter()
                .filter_map(|slot_id| leases.remove(&slot_id))
                .collect();
            // 快照保留下来的槽，tick 串行契约保证本轮后续无需重新比较。
            let existing_slots: HashSet<_> = leases.keys().copied().collect();
            // 同时交出锁外析构列表与现有槽快照。
            (stale_leases, existing_slots)
        };
        // 在租约锁外注销旧 State 订阅，避免析构重入锁顺序。
        drop(stale_leases);
        // 在所有 Effect 租约锁外为缺失 State 构建新订阅租约。
        let mut new_leases = HashMap::new();
        // 预分配最终 generation 快照，避免发布锁内重复分配。
        let mut snapshots = Vec::with_capacity(next_deps.len());
        // 为每个去重后的依赖建立快照，并仅为缺失槽执行订阅回调。
        for (slot_id, dep) in next_deps {
            // 缺失槽先注册后核对 generation，避免遗漏读取后的写入。
            if !existing_slots.contains(&slot_id) {
                // 由不可复制租约取得唯一的注销责任，回调期间不持有 Effect 锁。
                let lease =
                    (dep.subscribe_pending)(self.inner.pending.clone(), dep.observed_generation);
                // 暂存新租约，等待短锁插入当前 Effect 映射。
                new_leases.insert(slot_id, lease);
            }
            // 保存读取时观察到的 generation，而不是延后重新读取。
            snapshots.push((dep.check_generation, dep.observed_generation));
        }
        // 在短租约锁内插入新项，并把意外替换值移出锁区。
        let replaced_leases = {
            // tick 串行化保证通常无替换值，但仍防御并移出所有替换项。
            let mut leases = self
                .inner
                .leases
                .write()
                .unwrap_or_else(|error| error.into_inner());
            // 收集插入时被替换的旧租约，绝不在当前锁内析构。
            new_leases
                .into_iter()
                .filter_map(|(slot_id, lease)| leases.insert(slot_id, lease))
                .collect::<Vec<_>>()
        };
        // 在租约锁外释放任何防御性替换值。
        drop(replaced_leases);
        // 在独立短依赖锁中发布本轮 generation 快照。
        *self
            .inner
            .deps
            .write()
            .unwrap_or_else(|error| error.into_inner()) = snapshots;
    }

    /// 检查依赖是否有变化，如有则重新执行。
    /// 返回 `true` 表示重新执行了。
    pub fn tick(&self) -> bool {
        // 只有一个竞争者可获得本轮执行权，失败者不能清除 pending。
        if self
            .inner
            .running
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            // 另一轮仍在运行时保留 State 通知给随后 tick。
            return false;
        }
        // 让任何提前返回或 unwind 都可靠释放执行权。
        let _running = RunningLease {
            // 借用本 Effect 的运行标志作为 RAII 资源。
            running: &self.inner.running,
        };
        // 只有获得执行权的本轮可以消费当前 pending 信号。
        if !self.inner.pending.swap(false, Ordering::AcqRel) {
            // 没有待处理通知时无需读取依赖或执行用户闭包。
            return false;
        }
        // 在依赖快照锁下判断直接 State generation 是否已经变化。
        let need_run = {
            // 仅短暂读取依赖快照，不在其中执行用户代码。
            let deps = self
                .inner
                .deps
                .read()
                .unwrap_or_else(|error| error.into_inner());
            // 任何 generation 不一致都要求重新捕获依赖。
            deps.iter().any(|(check_generation, observed_generation)| {
                check_generation() != *observed_generation
            })
        };
        // generation 未变化时仅完成本轮 pending 消费。
        if !need_run {
            // 不执行用户闭包。
            return false;
        }
        // 在所有 State 与 Effect 内部锁外重新执行用户闭包并收集依赖。
        let (_, deps) = collect_deps(|| (self.inner.effect_fn)());
        // 用本轮读取结果更新订阅差集；运行期间的新通知保留在 pending。
        self.refresh_deps(deps);
        // 报告本轮确实执行了用户副作用。
        true
    }
}
