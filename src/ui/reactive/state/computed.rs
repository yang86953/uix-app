// 引入调试格式化与派生捕获指纹支持。
use std::fmt;
// 引入线程私有的重算环检测栈。
use std::cell::RefCell;
// 引入稳定指纹所需的默认哈希器。
use std::collections::hash_map::DefaultHasher;
// 引入类型与槽身份哈希契约。
use std::hash::{Hash, Hasher};
// 引入缓存与重算门并发原语。
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
// 引入共享所有权、条件变量和可恢复锁。
use std::sync::{Arc, Condvar, Mutex, RwLock};

// 引入绘制失效端点类型。
use crate::core::{Rect, WidgetId};
// 引入绘制失效队列句柄类型。
use crate::draw::renderer::InvalidationQueueHandle;

// 引入通用依赖订阅与租约私有契约。
use super::effect::{self, DependencySource, DependencySubscriber, EffectDependency, EffectLease};
// 引入父模块拥有的依赖追踪和绘制辅助函数。
use super::{
    NEXT_STATE_SLOT, PaintBindSite, StateSlotId, bind_persistent_paint_site, collect_deps,
    fire_paint_bindings, state_bind_capture_active, state_capture_active, track_dep,
};

// 保存当前线程正在执行用户计算函数的 Computed 槽路径。
thread_local! {
    // 每个线程拥有独立环检测栈，避免跨线程误判。
    static COMPUTED_EVALUATION_STACK: RefCell<Vec<StateSlotId>> = const { RefCell::new(Vec::new()) };
}

// 将缓存值和它对应的观察 revision 保持在同一把锁中。
struct ComputedCache<T> {
    // 保存本轮成功计算发布的值。
    value: T,
    // 保存本轮计算开始时的派生 revision。
    generation: u64,
}

// 保存单个 Computed 的重算执行权。
#[derive(Default)]
struct RecomputeGate {
    // 标记是否有线程正在锁外执行用户计算函数。
    running: bool,
}

// 保存 Computed 的计算、上游租约和下游订阅生命周期。
struct ComputedInner<T> {
    // 保存始终在所有内部锁外调用的用户计算函数。
    compute_fn: Arc<dyn Fn() -> T + Send + Sync>,
    // 原子保存缓存值与其 revision，禁止两者撕裂。
    cache: RwLock<Option<ComputedCache<T>>>,
    // 保存缓存读取上游时的 generation 快照。
    deps: RwLock<Vec<effect::GenerationSnapshot>>,
    // 保存每个上游槽唯一的精确释放租约。
    leases: RwLock<std::collections::HashMap<StateSlotId, EffectLease>>,
    // 保存不强持有 Effect 或嵌套 Computed 的下游表。
    subscribers: effect::DependencySubscriberRegistry,
    // 每次上游通知都递增的可订阅 revision。
    generation: AtomicU64,
    // 标记缓存是否需要下一次读取时重算。
    dirty: AtomicBool,
    // 串行化用户计算，且计算期间不持有此锁。
    gate: Mutex<RecomputeGate>,
    // 唤醒等待前一轮重算结束的读取者。
    gate_ready: Condvar,
    // 保存精确 Paint 失效端点。
    paint_sites: Arc<Mutex<Vec<PaintBindSite>>>,
    // 保存对外稳定的派生槽身份。
    slot_id: StateSlotId,
}

impl<T: Clone + Send + Sync + 'static> DependencySubscriber for ComputedInner<T> {
    // 上游变更只失效并传播，绝不主动计算。
    fn notify(&self) {
        // 每次通知均推进 revision，不能合并 clean 到 dirty 转换。
        self.generation.fetch_add(1, Ordering::AcqRel);
        // 让后续读取执行重算。
        self.dirty.store(true, Ordering::Release);
        // 在注册表锁内仅快照并清理存活下游。
        let subscribers = effect::collect_subscribers(&self.subscribers);
        // 释放注册表锁后同步继续失效链。
        effect::notify_subscribers(subscribers);
        // 通知当前绘制端点刷新派生值。
        fire_paint_bindings(&self.paint_sites);
    }
}

impl<T: Clone + Send + Sync + 'static> DependencySource for ComputedInner<T> {
    // 派生源的可订阅 generation 由无锁 revision 提供。
    fn generation(&self) -> u64 {
        self.generation.load(Ordering::Acquire)
    }

    // 下游表与缓存、重算门保持独立，租约析构不会进入用户计算锁域。
    fn subscribers(&self) -> &effect::DependencySubscriberRegistry {
        &self.subscribers
    }
}

// 覆盖计算 panic 的重算门释放与脏标记恢复。
struct RecomputeLease<'a, T> {
    // 借用当前派生内部状态。
    inner: &'a ComputedInner<T>,
    // 标记缓存和租约是否已经成功发布。
    completed: bool,
}

// 在用户计算期间持有一个线程私有槽路径条目。
struct EvaluationStackLease {
    // 保存本轮被压入的槽身份。
    slot_id: StateSlotId,
}

impl EvaluationStackLease {
    // 在可能等待重算门前拒绝当前线程已激活的同槽。
    fn assert_absent(slot_id: StateSlotId) {
        // 检查线程私有路径但不修改栈。
        COMPUTED_EVALUATION_STACK.with(|stack| {
            // 读取当前线程的重算路径。
            let stack = stack.borrow();
            // 当前槽已在路径中时绝不能等待自身运行权。
            assert!(
                !stack.contains(&slot_id),
                "检测到 Computed 递归重算环，槽 {} 不能等待自身",
                slot_id.get()
            );
        });
    }

    // 在执行用户函数前拒绝当前线程的自环或互环重入。
    fn enter(slot_id: StateSlotId) -> Self {
        // 先完成不修改栈的环检测。
        Self::assert_absent(slot_id);
        // 原子压入当前线程的重算路径。
        COMPUTED_EVALUATION_STACK.with(|stack| {
            // 独占访问线程私有栈。
            let mut stack = stack.borrow_mut();
            // 登记本轮用户计算所属槽。
            stack.push(slot_id);
        });
        // 返回覆盖 panic 路径的栈守卫。
        Self { slot_id }
    }
}

impl Drop for EvaluationStackLease {
    // 无论用户函数正常返回或 panic 都归还线程私有路径条目。
    fn drop(&mut self) {
        // 仅弹出本守卫在正常嵌套顺序下压入的栈顶条目。
        COMPUTED_EVALUATION_STACK.with(|stack| {
            // 独占访问线程私有栈。
            let mut stack = stack.borrow_mut();
            // 验证重算路径的 LIFO 生命周期不变量。
            let popped = stack.pop();
            // 失配说明内部守卫生命周期已被破坏。
            assert_eq!(popped, Some(self.slot_id), "Computed 重算栈释放顺序错误");
        });
    }
}

impl<T> RecomputeLease<'_, T> {
    // 标记正常发布已完成。
    fn complete(&mut self) {
        // 避免析构路径重复将成功计算标脏。
        self.completed = true;
    }
}

impl<T> Drop for RecomputeLease<'_, T> {
    // 无论返回或 unwind 都释放计算执行权。
    fn drop(&mut self) {
        // panic 时不能把旧缓存误标为干净。
        if !self.completed {
            // 保留下一次读取的重算请求。
            self.inner.dirty.store(true, Ordering::Release);
        }
        // 获取仅管理运行标记的短锁。
        let mut gate = self
            .inner
            .gate
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // 归还唯一计算执行权。
        gate.running = false;
        // 在释放 gate 锁后唤醒所有等待读取者。
        drop(gate);
        // 等待者必须重新判断 dirty 或 force 语义。
        self.inner.gate_ready.notify_all();
    }
}

/// 自动追踪上游、支持下游订阅的派生响应式值。
// 保持既有可复制公开句柄契约。
pub struct Computed<T> {
    // 所有克隆统一持有相同缓存、租约和下游表。
    inner: Arc<ComputedInner<T>>,
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    /// 创建并立即计算一次派生值以保持既有构造语义。
    pub fn new<F: Fn() -> T + Send + Sync + 'static>(f: F) -> Self {
        // 构造尚未发布缓存的统一内部对象。
        let computed = Self {
            // 初始化所有私有生命周期字段。
            inner: Arc::new(ComputedInner {
                // 提升用户函数为共享所有权。
                compute_fn: Arc::new(f),
                // 首次重算将发布完整缓存条目。
                cache: RwLock::new(None),
                // 首次重算将发布上游快照。
                deps: RwLock::new(Vec::new()),
                // 首次重算将安装上游租约。
                leases: RwLock::new(std::collections::HashMap::new()),
                // 建立与缓存锁完全分离的下游表。
                subscribers: RwLock::new(effect::DependencySubscribers::new()),
                // 初始 revision 为零。
                generation: AtomicU64::new(0),
                // 初始必须执行首次计算。
                dirty: AtomicBool::new(true),
                // 初始没有线程在运行计算函数。
                gate: Mutex::new(RecomputeGate::default()),
                // 建立等待前一轮计算完成的条件变量。
                gate_ready: Condvar::new(),
                // 建立私有精确 Paint 端点集合。
                paint_sites: Arc::new(Mutex::new(Vec::new())),
                // 分配对外稳定的派生槽身份。
                slot_id: StateSlotId(NEXT_STATE_SLOT.fetch_add(1, Ordering::Relaxed)),
            }),
        };
        // 首次强制计算以保持构造后缓存可用。
        let _ = computed.value_with_generation(true);
        // 返回已安装上游租约的公开句柄。
        computed
    }

    /// 绑定派生值的精确 Paint 失效端点。
    pub fn bind_paint_invalidation(
        &self,
        widget_id: WidgetId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        // 公开直接绑定保留既有持续站点语义。
        bind_persistent_paint_site(&self.inner.paint_sites, widget_id, queue, rect);
    }

    // 增加一份由实际节点拥有的派生绘制订阅。
    pub(super) fn bind_paint_site_invalidation(
        // 借用当前派生源。
        &self,
        // 接收实际组件的代际身份。
        widget_id: WidgetId,
        // 接收所属窗口失效队列。
        queue: InvalidationQueueHandle,
        // 接收当前精确绘制范围。
        rect: Option<Rect>,
    ) {
        // 在派生源共享站点集合中增加节点租约计数。
        super::retain_paint_site(&self.inner.paint_sites, widget_id, queue, rect);
    }

    // 释放一份实际节点拥有的派生绘制订阅。
    pub(super) fn unbind_paint_site_invalidation(
        // 借用当前派生源。
        &self,
        // 接收正在离开的组件身份。
        widget_id: WidgetId,
        // 接收用于区分窗口端点的失效队列。
        queue: &InvalidationQueueHandle,
    ) {
        // 最后一份租约离开时移除站点和窗口队列强引用。
        super::release_paint_site(&self.inner.paint_sites, widget_id, queue);
    }

    // 增加一份由实际节点拥有的派生布局订阅。
    pub(super) fn bind_layout_site_invalidation(
        &self,
        widget_id: WidgetId,
        queue: InvalidationQueueHandle,
    ) {
        super::retain_layout_site(&self.inner.paint_sites, widget_id, queue);
    }

    // 释放一份实际节点拥有的派生布局订阅。
    pub(super) fn unbind_layout_site_invalidation(
        &self,
        widget_id: WidgetId,
        queue: &InvalidationQueueHandle,
    ) {
        super::release_layout_site(&self.inner.paint_sites, widget_id, queue);
    }

    /// 返回对外稳定的派生槽身份。
    pub fn slot_id(&self) -> StateSlotId {
        // 槽身份在整个 Arc 生命周期内保持不变。
        self.inner.slot_id
    }

    // 为 View 捕获去重提供不暴露计算函数的稳定指纹。
    #[allow(dead_code)]
    pub(crate) fn capture_fingerprint(&self) -> u64 {
        // 创建默认哈希器。
        let mut hasher = DefaultHasher::new();
        // 混入派生值类型。
        std::any::TypeId::of::<T>().hash(&mut hasher);
        // 混入稳定槽身份。
        self.slot_id().hash(&mut hasher);
        // 返回最终指纹。
        hasher.finish()
    }

    // 在单锁中读取完整缓存条目。
    fn cache_entry(&self) -> Option<ComputedCache<T>> {
        // 在缓存读锁内同时克隆值和 revision。
        self.inner
            .cache
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .as_ref()
            .map(|entry| ComputedCache {
                // 克隆缓存值离开锁区。
                value: entry.value.clone(),
                // 复制与值同锁读取的 revision。
                generation: entry.generation,
            })
    }

    // 读取缓存，必要时等待或串行重算，并返回一致条目。
    fn value_with_generation(&self, force: bool) -> (T, u64) {
        // 在检查缓存或等待重算门前拒绝当前线程的递归读取。
        EvaluationStackLease::assert_absent(self.inner.slot_id);
        // force 必须在等待前一轮完成后仍执行自己的探测轮。
        // 循环仅围绕条件变量等待与状态重判，不进行忙等。
        loop {
            // 获取只管理重算所有权的短锁。
            let mut gate = self
                .inner
                .gate
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            // 等待现有计算完成，绝不返回失效旧缓存。
            while gate.running {
                // 条件变量释放 gate 锁并在唤醒后重新获得它。
                gate = self
                    .inner
                    .gate_ready
                    .wait(gate)
                    .unwrap_or_else(|error| error.into_inner());
            }
            // 等待者若不是 force，先重新判断前一轮是否已清除 dirty。
            if !force && !self.inner.dirty.load(Ordering::Acquire) {
                // 保持 gate 锁读取缓存，阻止其他线程在返回前启动重算。
                if let Some(entry) = self.cache_entry() {
                    // 释放重算门后返回一致缓存条目。
                    drop(gate);
                    return (entry.value, entry.generation);
                }
                // 缓存缺失时继续尝试取得计算权。
                drop(gate);
                continue;
            }
            // 当前线程取得唯一计算权。
            gate.running = true;
            // 计算期间不持有 gate 锁。
            drop(gate);
            // 守卫覆盖用户计算 panic 并唤醒等待者。
            let mut lease = RecomputeLease {
                // 借用统一内部对象。
                inner: &self.inner,
                // 初始尚未成功发布。
                completed: false,
            };
            // 先消费旧 dirty，再读取 revision，避免覆盖已发生通知。
            self.inner.dirty.store(false, Ordering::Release);
            // 取得本轮计算开始 revision。
            let revision = self.inner.generation.load(Ordering::Acquire);
            // 在执行用户函数前登记线程私有重算路径。
            let _evaluation = EvaluationStackLease::enter(self.inner.slot_id);
            // 用户函数和依赖收集都在所有内部锁外执行。
            let (value, deps) = collect_deps(|| (self.inner.compute_fn)());
            // 将当前 Computed 作为其上游的私有下游订阅者。
            let subscriber: Arc<dyn DependencySubscriber> = self.inner.clone();
            // 安装差集租约时不持有缓存或重算门锁。
            effect::refresh_dependency_leases(
                deps,
                &self.inner.leases,
                &self.inner.deps,
                subscriber,
            );
            // 在单个写锁内同时发布值与本轮 revision。
            *self
                .inner
                .cache
                .write()
                .unwrap_or_else(|error| error.into_inner()) = Some(ComputedCache {
                // 保存本轮计算结果。
                value: value.clone(),
                // 保存与结果不可分离的起始 revision。
                generation: revision,
            });
            // 标记成功发布，析构仅释放计算权。
            lease.complete();
            // 返回刚发布的一致条目。
            return (value, revision);
        }
    }

    /// 读取派生值并将其作为可订阅依赖登记到外层。
    pub fn get(&self) -> T {
        // 继续支持现有 Paint 绑定捕获。
        super::try_capture_computed_bind(self);
        // 构建或探测阶段必须额外执行一次 force 重算。
        // View 构建或组件绘制捕获期间都必须重新暴露当前派生依赖。
        let force = state_capture_active() || state_bind_capture_active();
        // 先取得缓存与其不可分离的 observed generation。
        let (value, observed_generation) = self.value_with_generation(force);
        // 保存稳定派生槽身份。
        let slot_id = self.inner.slot_id;
        // 将派生源本身登记给外层 Computed 或 Effect。
        track_dep(slot_id, || {
            // 仅首次读取克隆统一派生源，不再为检查与订阅入口分别装箱闭包。
            let source: Arc<dyn DependencySource> = self.inner.clone();
            EffectDependency {
                // 使用派生值自身的稳定槽。
                slot_id,
                // 使用单锁缓存条目提供的 observed revision。
                observed_generation,
                // 统一源契约保留注册后 revision 复核与精确租约释放。
                source,
            }
        });
        // 返回与 observed revision 同轮的派生值。
        value
    }

    /// 同步失效并完成至少一轮强制重算。
    pub fn invalidate(&self) {
        // 先推进 revision、置脏并同步通知下游。
        self.inner.notify();
        // 首轮必须强制重算。
        let _ = self.value_with_generation(true);
    }

    // 暴露私有下游计数供生命周期回归验证。
    #[cfg(test)]
    // 不形成公开 API。
    pub(crate) fn effect_subscriber_count(&self) -> usize {
        // 委托通用弱订阅表统计。
        effect::subscriber_count(&self.inner.subscribers)
    }

    // 暴露私有绘制端点计数供节点生命周期回归验证。
    #[cfg(test)]
    // 不形成公开 Computed API。
    pub(crate) fn paint_site_count(&self) -> usize {
        // 读取派生源当前仍保留的绘制端点数量。
        self.inner
            .paint_sites
            .lock()
            .map(|sites| sites.len())
            .unwrap_or_default()
    }

    // 读取同锁缓存条目供私有并发回归验证。
    #[cfg(test)]
    // 不形成公开 API。
    pub(crate) fn cache_snapshot(&self) -> (T, u64) {
        // 构造后缓存必定存在，若 panic 则暴露违反的内部不变量。
        let entry = self.cache_entry().expect("Computed 缓存应在构造后存在");
        // 返回同一锁快照中的值和 revision。
        (entry.value, entry.generation)
    }
}

impl<T> Clone for Computed<T> {
    // 克隆公开句柄只增加内部 Arc 引用计数，不要求派生值可克隆。
    fn clone(&self) -> Self {
        // 复用同一份缓存、租约和下游注册表。
        Self {
            // 克隆唯一内部所有权句柄。
            inner: self.inner.clone(),
        }
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for Computed<T> {
    // 输出稳定槽身份与当前解析后的派生值。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 保持既有结构化 Debug 契约。
        f.debug_struct("Computed")
            // 输出稳定槽身份。
            .field("slot_id", &self.slot_id())
            // 输出按当前失效状态解析后的值。
            .field("value", &self.get())
            // 完成格式化。
            .finish()
    }
}
