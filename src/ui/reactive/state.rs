use std::any::TypeId;
// 使用线程局部可变容器保存嵌套捕获上下文。
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::core::{ComponentId, Rect};
use crate::draw::renderer::{invalidate_paint_handle, InvalidationQueueHandle};

type ReconcileCallback = Arc<dyn Fn() + Send + Sync>;
type GenerationCheck = Box<dyn Fn() -> u64 + Send + Sync>;
type GenerationSnapshot = (GenerationCheck, u64);
type StateWatcher<T> = Arc<dyn Fn(&T) + Send + Sync>;
type StateBindCapture = (
    ComponentId,
    InvalidationQueueHandle,
    Option<Rect>,
    Vec<Arc<dyn StatePaintBind>>,
);

// 拆分结构性绑定租约，保持响应式状态主体低于规模上限。
#[path = "state/reconcile_lease.rs"]
// 编译结构性绑定租约的私有实现模块。
mod reconcile_lease;
// 向树与节点生命周期边界暴露租约类型。
pub(crate) use reconcile_lease::ReconcileBindLease;

#[derive(Clone)]
pub(crate) struct PaintBindSite {
    component_id: ComponentId,
    queue: InvalidationQueueHandle,
    rect: Option<Rect>,
}

#[derive(Clone)]
pub(crate) struct ReconcileBindSite {
    key: usize,
    callback: ReconcileCallback,
    // 记录当前树根和节点持有的窄绑定租约数量。
    leases: usize,
}


// ── 响应式依赖追踪 ────────────────────────────────────────────
//
// 设计：使用 thread_local 追踪当前正在计算的 Computed 所读取的 State。
// State::get() 在追踪启用时自动注册依赖，Computed 在计算完毕后收集
// 这些依赖的 generation 快照，后续 get() 时比对以判断是否需要重新计算。

thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static TRACKING_DEPS: RefCell<Option<Vec<EffectDependency>>> =
        const { RefCell::new(None) };
}

struct EffectDependency {
    slot_id: StateSlotId,
    check_generation: GenerationCheck,
    subscribe_pending: Box<dyn Fn(Arc<AtomicBool>) + Send + Sync>,
}

// layout 后探测 DynamicLabel 闭包时捕获 `State::get()` 读取的实例。
thread_local! {
    static STATE_BIND_CAPTURE: RefCell<Option<StateBindCapture>> = const { RefCell::new(None) };
}

// 保存一次 View 构建捕获的结构依赖与副作用，并由声明根显式交接给所属树。
#[derive(Default)]
pub(crate) struct StateCaptureOutput {
    // 保存本帧已登记的槽身份以保持同帧订阅去重。
    state_bind_slots: Vec<StateSlotId>,
    // 保存去重后的结构性 State 绑定源。
    pub(crate) state_binds: Vec<Arc<dyn StatePaintBind>>,
    // 保存本次构建创建的 Effect 实例。
    pub(crate) effects: Vec<Effect>,
}

// 使用线程私有的捕获帧栈隔离嵌套 View 构建与不同窗口的同步构建。
thread_local! {
    // 每个 begin 都压入独立帧，finish 或 panic 只弹出栈顶。
    static STATE_CAPTURE_STACK: RefCell<Vec<StateCaptureOutput>> = const { RefCell::new(Vec::new()) };
}
static NEXT_STATE_SLOT: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateSlotId(pub(crate) u64);

impl StateSlotId {
    pub fn get(self) -> u64 {
        self.0
    }
}

/// 开始捕获 `State::get` 依赖 / `Effect::new` 实例（View 构建期间调用）。
pub(crate) fn begin_state_capture() {
    // 为当前构建创建独立输出帧，避免嵌套构建清空外层结果。
    STATE_CAPTURE_STACK.with(|stack| stack.borrow_mut().push(StateCaptureOutput::default()));
}

/// 结束 View 构建期的 State 捕获，并返回当前帧专属输出。
pub(crate) fn end_state_capture() -> StateCaptureOutput {
    // 只取走栈顶帧，外层捕获在嵌套完成后继续保持活动。
    STATE_CAPTURE_STACK.with(|stack| stack.borrow_mut().pop().unwrap_or_default())
}

// 判断当前线程是否仍有任意活动 View 捕获帧，供 Computed 强制重新暴露底层依赖。
fn state_capture_active() -> bool {
    // 仅检查栈是否非空，嵌套帧结束后外层帧仍应视为活动。
    STATE_CAPTURE_STACK.with(|stack| !stack.borrow().is_empty())
}

/// 开始探测组件测量或绘制时读取的 State / Computed。
pub fn begin_state_bind_capture(
    component_id: ComponentId,
    queue: InvalidationQueueHandle,
    rect: Option<Rect>,
) {
    STATE_BIND_CAPTURE.with(|c| {
        *c.borrow_mut() = Some((component_id, queue, rect, Vec::new()));
    });
}

/// 结束探测并将捕获到的 State 绑定到指定 widget（仅 Paint，不 reconcile）。
pub fn end_state_bind_capture(component_id: ComponentId) {
    STATE_BIND_CAPTURE.with(|c| {
        let Some((id, queue, rect, states)) = c.borrow_mut().take() else {
            return;
        };
        if id != component_id {
            tracing::warn!("State 绑定探测 component_id 不一致: 期望 {component_id}, 实际 {id}");
        }
        for source in states {
            // 渲染闭包或组件在 render/measure 时读取最新值，只需 Paint。
            // 若再绑 reconcile，周期状态会反复触发整树 reconcile+layout。
            source.bind_paint(component_id, queue.clone(), rect);
        }
    });
}

fn try_capture_state_bind<T: Clone + Send + Sync + 'static>(state: &State<T>) {
    STATE_BIND_CAPTURE.with(|c| {
        let mut guard = c.borrow_mut();
        if let Some((_, _, _, ref mut captured)) = *guard {
            let bind: Arc<dyn StatePaintBind> = Arc::new(state.clone());
            captured.push(bind);
        }
    });
}

pub(crate) fn capture_pending_state_bind<T: Clone + Send + Sync + 'static>(state: &State<T>) {
    // 只向当前最内层构建帧登记读取，避免子 View 输出泄漏到父 View。
    STATE_CAPTURE_STACK.with(|stack| {
        // 取得可变栈顶以维护本帧的去重集合。
        let mut stack = stack.borrow_mut();
        // 没有活跃 View 捕获时保持既有无副作用读取语义。
        let Some(output) = stack.last_mut() else {
            // 直接结束以避免为非 View 读取分配绑定。
            return;
        };
        // 读取稳定槽身份用于同帧去重。
        let slot_id = state.slot_id();
        // 已登记的同槽 State 不应重复生成 reconcile 订阅。
        if output.state_bind_slots.contains(&slot_id) {
            // 同槽已经属于本帧输出，避免重复安装相同 reconcile 订阅。
            return;
        }
        // 为当前 State 生成交给根树绑定的窄接口句柄。
        let bind: Arc<dyn StatePaintBind> = Arc::new(state.clone());
        // 先登记槽身份，使后续同帧读取保持去重。
        output.state_bind_slots.push(slot_id);
        // 记录本帧读取的 State 绑定源。
        output.state_binds.push(bind);
    });
}

fn try_capture_computed_bind<T: Clone + Send + Sync + 'static>(computed: &Computed<T>) {
    STATE_BIND_CAPTURE.with(|c| {
        if let Some((component_id, queue, rect, _)) = c.borrow().as_ref() {
            computed.bind_paint_invalidation(*component_id, queue.clone(), *rect);
        }
    });
}

fn bind_paint_site(
    sites: &Arc<std::sync::Mutex<Vec<PaintBindSite>>>,
    component_id: ComponentId,
    queue: InvalidationQueueHandle,
    rect: Option<Rect>,
) {
    if let Ok(mut guard) = sites.lock() {
        if let Some(site) = guard
            .iter_mut()
            .find(|site| site.component_id == component_id && Arc::ptr_eq(&site.queue, &queue))
        {
            site.rect = rect;
        } else {
            guard.push(PaintBindSite {
                component_id,
                queue,
                rect,
            });
        }
    }
}

fn fire_paint_bindings(sites: &Arc<std::sync::Mutex<Vec<PaintBindSite>>>) {
    let sites = sites.lock().ok().map(|guard| guard.clone());
    let Some(sites) = sites else {
        return;
    };
    for site in &sites {
        invalidate_paint_handle(&site.queue, site.component_id, site.rect);
    }
}

fn bind_reconcile_site(
    sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
    key: usize,
    callback: ReconcileCallback,
) {
    if let Ok(mut guard) = sites.lock() {
        if let Some(site) = guard.iter_mut().find(|site| site.key == key) {
            // 刷新同一树请求端口的可调用句柄。
            site.callback = callback;
            // 增加同源同树的生命周期持有计数。
            site.leases = site.leases.saturating_add(1);
        } else {
            // 建立由第一份租约持有的树请求端口。
            guard.push(ReconcileBindSite {
                // 保存树请求端口键。
                key,
                // 保存树请求端口回调。
                callback,
                // 记录第一份生命周期租约。
                leases: 1,
            });
        }
    }
}

// 撤销一份同源同树的结构性 State 绑定租约。
fn unbind_reconcile_site(
    // 接收 State 内部的树请求端口集合。
    sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
    // 接收需要释放的 WidgetTree 请求端口键。
    key: usize,
) {
    // 仅在站点集合仍可访问时执行精确计数递减。
    if let Ok(mut guard) = sites.lock() {
        // 找到同一树的站点并减少一份租约。
        if let Some(site) = guard.iter_mut().find(|site| site.key == key) {
            // 防御性饱和减法避免异常重复析构下溢。
            site.leases = site.leases.saturating_sub(1);
        }
        // 移除已经不再由任何树或节点持有的站点。
        guard.retain(|site| site.leases != 0);
    }
}

fn fire_reconcile_bindings(sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>) {
    let callbacks: Vec<ReconcileCallback> = sites
        .lock()
        .ok()
        .map(|guard| guard.iter().map(|site| site.callback.clone()).collect())
        .unwrap_or_default();
    for callback in callbacks {
        callback();
    }
}

/// State 变更时推送精确 Paint 失效的绑定接口。
pub trait StatePaintBind: Send + Sync {
    fn bind_reconcile(&self, reconcile: ReconcileCallback);
    fn bind_reconcile_site(&self, _key: usize, reconcile: ReconcileCallback) {
        self.bind_reconcile(reconcile);
    }
    // 撤销一份结构性树订阅；不支持订阅的源保持无操作。
    fn unbind_reconcile_site(&self, _key: usize) {}
    fn bind_paint(
        &self,
        component_id: ComponentId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    );
}

impl<T: Clone + Send + Sync + 'static> StatePaintBind for State<T> {
    fn bind_reconcile(&self, reconcile: ReconcileCallback) {
        self.bind_reconcile_invalidation(0, reconcile);
    }

    fn bind_reconcile_site(&self, key: usize, reconcile: ReconcileCallback) {
        self.bind_reconcile_invalidation(key, reconcile);
    }

    // 释放一份同树结构性订阅租约。
    fn unbind_reconcile_site(&self, key: usize) {
        self.unbind_reconcile_invalidation(key);
    }

    fn bind_paint(
        &self,
        component_id: ComponentId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        self.bind_paint_invalidation(component_id, queue, rect);
    }
}

impl<T: Clone + Send + Sync + 'static> StatePaintBind for Computed<T> {
    fn bind_reconcile(&self, _reconcile: ReconcileCallback) {}

    fn bind_paint(
        &self,
        component_id: ComponentId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        self.bind_paint_invalidation(component_id, queue, rect);
    }
}

// 保存一次依赖追踪调用替换掉的外层上下文，并在离开作用域时归还它。
struct DependencyTrackingGuard {
    // 保存进入本层前的外层依赖收集器。
    outer: Option<Vec<EffectDependency>>,
    // 标记外层上下文是否已经归还，避免析构时重复覆盖。
    restored: bool,
}

impl DependencyTrackingGuard {
    // 安装一个只属于当前闭包的空依赖收集器。
    fn enter() -> Self {
        // 从线程局部存储中原子地替换当前追踪上下文。
        let outer = TRACKING_DEPS.with(|deps| {
            // 独占访问当前线程的追踪上下文。
            let mut deps = deps.borrow_mut();
            // 暂存可能存在的外层收集器。
            let outer = deps.take();
            // 安装本层独立的空收集器。
            *deps = Some(Vec::new());
            // 将外层收集器交给守卫保存。
            outer
        });
        // 返回负责恢复外层上下文的守卫。
        Self {
            // 记录进入时摘下的上下文。
            outer,
            // 守卫初始尚未执行恢复。
            restored: false,
        }
    }

    // 取出本层收集结果，并立即归还进入前的上下文。
    fn finish(mut self) -> Vec<EffectDependency> {
        // 从当前线程取出本层独立收集器。
        let collected = TRACKING_DEPS.with(|deps| {
            // 独占访问当前线程的追踪上下文。
            let mut deps = deps.borrow_mut();
            // 取走本层的收集结果；异常重入时退化为空集合。
            deps.take().unwrap_or_default()
        });
        // 在返回结果前归还外层上下文。
        self.restore();
        // 将本层依赖交给调用方建立 generation 快照。
        collected
    }

    // 将进入本层前的上下文原样写回线程局部存储。
    fn restore(&mut self) {
        // 已恢复时不再覆盖可能已安装的新上下文。
        if self.restored {
            // 直接结束幂等恢复。
            return;
        }
        // 取出外层上下文，包含“外层不存在”的 None 情形。
        let outer = self.outer.take();
        // 用进入前的上下文替换当前本层上下文。
        TRACKING_DEPS.with(|deps| {
            // 独占访问当前线程的追踪上下文。
            let mut deps = deps.borrow_mut();
            // 恢复外层追踪器或明确清空追踪状态。
            *deps = outer;
        });
        // 标记析构不应再次恢复。
        self.restored = true;
    }
}

impl Drop for DependencyTrackingGuard {
    // 覆盖闭包 unwind 路径，确保 panic 不会泄漏或丢失外层收集器。
    fn drop(&mut self) {
        // 无论正常路径还是 panic 路径，都幂等地恢复外层上下文。
        self.restore();
    }
}

/// 在当前线程启用依赖追踪，执行闭包后返回收集到的依赖 generation 检查器列表。
/// 支持嵌套：内层 collect_deps 保存并恢复外层追踪上下文，使 `Computed` 在其 get()
/// 内部也能被外层正确追踪。
fn collect_deps<F, R>(f: F) -> (R, Vec<EffectDependency>)
where
    F: FnOnce() -> R,
{
    // 在执行用户闭包前安装可在 unwind 时自动恢复的上下文守卫。
    let guard = DependencyTrackingGuard::enter();
    // 执行实际的 Computed 或 Effect 依赖读取。
    let result = f();
    // 仅在正常返回时交出本层完整的依赖集合。
    let collected = guard.finish();
    // 返回闭包结果及其对应的独立依赖集合。
    (result, collected)
}

/// 将当前 State 注册到追踪上下文中（如果追踪已启用）。
fn track_dep<F>(register: F)
where
    F: FnOnce() -> EffectDependency,
{
    TRACKING_DEPS.with(|deps| {
        let mut deps = deps.borrow_mut();
        if let Some(ref mut list) = *deps {
            list.push(register());
        }
    });
}

/// A reactive state value that notifies watchers on change.
/// Thread-safe: Send + Sync when T is Send + Sync.
///
/// 支持自动 reconcile invalidation：当通过 `set()` / `update()` 修改值时，自动调用注册的 reconcile 回调，
/// 通知 WidgetTree 重新渲染所属 View。reconcile 回调由 ViewAdapter 在 ViewNode 展开时自动绑定，
/// 用户不需要手动请求 reconcile。
pub struct State<T> {
    inner: Arc<RwLock<StateInner<T>>>,
    /// reconcile invalidation 回调——值变更时自动调用，通知 WidgetTree 重绘所属节点。
    pub(crate) reconcile_sites: Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
    /// Phase 6：精确 Paint 失效绑定（ComponentId + 队列句柄）。
    pub(crate) paint_sites: Arc<std::sync::Mutex<Vec<PaintBindSite>>>,
}

struct StateInner<T> {
    slot_id: StateSlotId,
    value: T,
    generation: u64,
    #[allow(clippy::type_complexity)]
    watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>,
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    pub fn new(value: T) -> Self {
        let reconcile_sites = Arc::new(std::sync::Mutex::new(Vec::new()));
        let paint_sites = Arc::new(std::sync::Mutex::new(Vec::new()));

        Self {
            inner: Arc::new(RwLock::new(StateInner {
                slot_id: StateSlotId(NEXT_STATE_SLOT.fetch_add(1, Ordering::Relaxed)),
                value,
                generation: 0,
                watchers: Vec::new(),
            })),
            reconcile_sites,
            paint_sites,
        }
    }

    /// 绑定精确 Paint 失效：State 变更时向队列推送 `Invalidation::Paint`。
    pub fn bind_paint_invalidation(
        &self,
        component_id: ComponentId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        bind_paint_site(&self.paint_sites, component_id, queue, rect);
    }

    pub fn bind_reconcile_invalidation(&self, key: usize, reconcile: ReconcileCallback) {
        bind_reconcile_site(&self.reconcile_sites, key, reconcile);
    }

    // 释放一份由树或节点生命周期持有的结构性订阅。
    pub fn unbind_reconcile_invalidation(&self, key: usize) {
        // 从同树站点扣除当前租约。
        unbind_reconcile_site(&self.reconcile_sites, key);
    }

    /// 设置 reconcile invalidation 回调。此回调在值变更时（`set` / `update`）自动调用。
    /// 由 ViewAdapter 内部使用，用户不需要调用此方法。
    pub fn set_reconcile_invalidation_fn<F: Fn() + Send + Sync + 'static>(&self, f: F) {
        if let Ok(mut guard) = self.reconcile_sites.lock() {
            guard.clear();
            guard.push(ReconcileBindSite {
                key: 0,
                callback: Arc::new(f),
                // 兼容入口直接建立一份永久到下次覆盖的持有。
                leases: 1,
            });
        }
    }

    pub fn get(&self) -> T {
        // 将自身注册到活跃的追踪上下文中（如 Computed 计算期间）
        let self_clone = self.inner.clone();
        let subscribe_inner = self.inner.clone();
        let slot_id = self.slot_id();
        track_dep(move || EffectDependency {
            slot_id,
            check_generation: Box::new(move || {
                self_clone
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .generation
            }),
            subscribe_pending: Box::new(move |pending| {
                subscribe_inner
                    .write()
                    .unwrap_or_else(|e| e.into_inner())
                    .watchers
                    .push(Arc::new(move |_| {
                        pending.store(true, Ordering::Release);
                    }));
            }),
        });
        try_capture_state_bind(self);
        capture_pending_state_bind(self);

        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .value
            .clone()
    }

    pub(crate) fn get_untracked(&self) -> T {
        self.inner
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .value
            .clone()
    }

    pub fn set(&self, value: T) {
        let watchers: Vec<StateWatcher<T>>;
        let snapshot: T;
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.value = value;
            inner.generation += 1;
            snapshot = inner.value.clone();
            watchers = inner.watchers.clone();
        }
        for watcher in &watchers {
            watcher(&snapshot);
        }
        Self::fire_invalidation(&self.reconcile_sites, &self.paint_sites);
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let watchers: Vec<StateWatcher<T>>;
        let snapshot: T;
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            f(&mut inner.value);
            inner.generation += 1;
            snapshot = inner.value.clone();
            watchers = inner.watchers.clone();
        }
        for watcher in &watchers {
            watcher(&snapshot);
        }
        Self::fire_invalidation(&self.reconcile_sites, &self.paint_sites);
    }

    fn fire_invalidation(
        reconcile_sites: &Arc<std::sync::Mutex<Vec<ReconcileBindSite>>>,
        paint_sites: &Arc<std::sync::Mutex<Vec<PaintBindSite>>>,
    ) {
        fire_paint_bindings(paint_sites);
        fire_reconcile_bindings(reconcile_sites);
    }

    pub fn watch<F: Fn(&T) + Send + Sync + 'static>(&self, f: F) {
        self.inner
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .watchers
            .push(Arc::new(f));
    }

    pub fn generation(&self) -> u64 {
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .generation
    }

    pub fn slot_id(&self) -> StateSlotId {
        self.inner.read().unwrap_or_else(|e| e.into_inner()).slot_id
    }

    #[allow(dead_code)]
    pub(crate) fn capture_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        TypeId::of::<T>().hash(&mut hasher);
        self.slot_id().hash(&mut hasher);
        hasher.finish()
    }
}

impl<T: Clone + Send + Sync + 'static> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            reconcile_sites: self.reconcile_sites.clone(),
            paint_sites: self.paint_sites.clone(),
        }
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("slot_id", &self.slot_id())
            .field("value", &self.get())
            .field("generation", &self.generation())
            .finish()
    }
}

/// ── Computed — 自动追踪依赖的派生状态 ──────────────────────────
///
/// 在构造时执行一次闭包，自动记录所有读取过的 State 作为依赖。
/// 后续 `get()` 时比对依赖的 generation，如有变化则重新计算。
/// 无需手动调用 `invalidate()`。
///
/// # 示例
/// ```ignore
/// let a = State::new(1);
/// let b = State::new(2);
/// let sum = Computed::new(|| a.get() + b.get());
/// assert_eq!(sum.get(), 3);
/// a.set(10);
/// assert_eq!(sum.get(), 12); // 自动重新计算
/// ```
pub struct Computed<T> {
    slot_id: StateSlotId,
    compute_fn: Arc<dyn Fn() -> T + Send + Sync>,
    cached: Arc<RwLock<Option<T>>>,
    /// 依赖的 generation 检查器列表：(检查器, 上次计算时的 generation)
    deps: Arc<RwLock<Vec<GenerationSnapshot>>>,
    /// 自身 generation：值变更时递增，供外层计算/Effect 追踪本 Computed 的变化。
    generation: Arc<AtomicU64>,
    /// Phase R2：精确 Paint 失效绑定。
    paint_sites: Arc<std::sync::Mutex<Vec<PaintBindSite>>>,
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    /// 创建一个自动追踪依赖的 Computed。
    ///
    /// 构造时会立即执行一次 `f` 以收集依赖，之后 `get()` 自动判断是否需要重算。
    pub fn new<F: Fn() -> T + Send + Sync + 'static>(f: F) -> Self {
        let (initial, deps) = collect_deps(&f);
        let dep_pairs: Vec<_> = deps
            .into_iter()
            .map(|dep| {
                let r#gen = (dep.check_generation)();
                (dep.check_generation, r#gen)
            })
            .collect();

        Self {
            slot_id: StateSlotId(NEXT_STATE_SLOT.fetch_add(1, Ordering::Relaxed)),
            compute_fn: Arc::new(f),
            cached: Arc::new(RwLock::new(Some(initial))),
            deps: Arc::new(RwLock::new(dep_pairs)),
            generation: Arc::new(AtomicU64::new(0)),
            paint_sites: Arc::new(std::sync::Mutex::new(Vec::new())),
        }
    }

    /// 绑定精确 Paint 失效：依赖变化导致重算时向队列推送 `Invalidation::Paint`。
    pub fn bind_paint_invalidation(
        &self,
        component_id: ComponentId,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        bind_paint_site(&self.paint_sites, component_id, queue, rect);
    }

    pub fn slot_id(&self) -> StateSlotId {
        self.slot_id
    }

    #[allow(dead_code)]
    pub(crate) fn capture_fingerprint(&self) -> u64 {
        let mut hasher = DefaultHasher::new();
        TypeId::of::<T>().hash(&mut hasher);
        self.slot_id.hash(&mut hasher);
        hasher.finish()
    }

    pub fn get(&self) -> T {
        try_capture_computed_bind(self);

        // 将自身注册到活跃的追踪上下文中（外层 Computed/Effect 可捕获本 Computed 作为依赖）
        let self_gen = self.generation.clone();
        let self_slot = self.slot_id;
        track_dep(move || EffectDependency {
            slot_id: self_slot,
            check_generation: Box::new(move || self_gen.load(Ordering::Acquire)),
            subscribe_pending: Box::new(move |_pending| {
                // Computed 暂不订阅 pending 通知；依赖变化在 get() 内同步检测。
            }),
        });

        let force_probe =
            state_capture_active() || STATE_BIND_CAPTURE.with(|capture| capture.borrow().is_some());

        // 检查依赖是否变化；View 构建与 layout 探测阶段强制执行一次，
        // 使缓存中的 Computed 也能重新暴露底层 State 绑定。
        let need_recompute = if force_probe {
            true
        } else {
            let deps = self.deps.read().unwrap_or_else(|e| e.into_inner());
            deps.iter()
                .any(|(check, cached_gen)| check() != *cached_gen)
        };

        if need_recompute {
            let (value, new_deps) = collect_deps(|| (self.compute_fn)());
            let new_pairs: Vec<_> = new_deps
                .into_iter()
                .map(|dep| {
                    let r#gen = (dep.check_generation)();
                    (dep.check_generation, r#gen)
                })
                .collect();

            let mut cached = self.cached.write().unwrap_or_else(|e| e.into_inner());
            *cached = Some(value.clone());
            let mut deps = self.deps.write().unwrap_or_else(|e| e.into_inner());
            *deps = new_pairs;
            self.generation.fetch_add(1, Ordering::Release);
            fire_paint_bindings(&self.paint_sites);
            value
        } else {
            let cached = self.cached.read().unwrap_or_else(|e| e.into_inner());
            cached.clone().unwrap_or_else(|| (self.compute_fn)())
        }
    }

    /// 强制使缓存失效并重新计算（当依赖无法被自动追踪时使用）。
    pub fn invalidate(&self) {
        let (value, new_deps) = collect_deps(|| (self.compute_fn)());
        let new_pairs: Vec<_> = new_deps
            .into_iter()
            .map(|dep| {
                let r#gen = (dep.check_generation)();
                (dep.check_generation, r#gen)
            })
            .collect();
        let mut cached = self.cached.write().unwrap_or_else(|e| e.into_inner());
        *cached = Some(value);
        let mut deps = self.deps.write().unwrap_or_else(|e| e.into_inner());
        *deps = new_pairs;
        self.generation.fetch_add(1, Ordering::Release);
    }
}

impl<T> Clone for Computed<T> {
    fn clone(&self) -> Self {
        Self {
            slot_id: self.slot_id,
            compute_fn: self.compute_fn.clone(),
            cached: self.cached.clone(),
            deps: self.deps.clone(),
            generation: self.generation.clone(),
            paint_sites: self.paint_sites.clone(),
        }
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for Computed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Computed")
            .field("slot_id", &self.slot_id())
            .field("value", &self.get())
            .finish()
    }
}

/// ── Effect — 自动追踪依赖的副作用 ──────────────────────────────
struct EffectInner {
    effect_fn: Box<dyn Fn() + Send + Sync>,
    deps: RwLock<Vec<GenerationSnapshot>>,
    subscriptions: RwLock<HashSet<StateSlotId>>,
    pending: Arc<AtomicBool>,
}

/// 创建时执行闭包，自动追踪其中读取的所有 State。
/// 当任意依赖的 generation 变化时，`tick()` 重新执行。
#[derive(Clone)]
pub struct Effect {
    inner: Arc<EffectInner>,
}

impl Effect {
    pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
        let (_, deps) = collect_deps(&f);
        let effect = Self {
            inner: Arc::new(EffectInner {
                effect_fn: Box::new(f),
                deps: RwLock::new(Vec::new()),
                subscriptions: RwLock::new(HashSet::new()),
                pending: Arc::new(AtomicBool::new(false)),
            }),
        };
        effect.refresh_deps(deps);
        // 仅把新 Effect 交给当前最内层 View 构建帧。
        STATE_CAPTURE_STACK.with(|stack| {
            // 取得栈顶输出以保证嵌套构建彼此隔离。
            let mut stack = stack.borrow_mut();
            // 非 View 构建期创建的 Effect 继续保持未注册的既有行为。
            let Some(output) = stack.last_mut() else {
                // 没有活动捕获时无需向任何树登记。
                return;
            };
            // 保存本帧新建的 Effect，稍后随 ViewNode 显式交接。
            output.effects.push(effect.clone());
        });
        effect
    }

    pub fn has_pending(&self) -> bool {
        self.inner.pending.load(Ordering::Acquire)
    }

    fn refresh_deps(&self, deps: Vec<EffectDependency>) {
        let mut dep_pairs = Vec::with_capacity(deps.len());
        let mut subscriptions = self
            .inner
            .subscriptions
            .write()
            .unwrap_or_else(|e| e.into_inner());

        for dep in deps {
            if subscriptions.insert(dep.slot_id) {
                (dep.subscribe_pending)(self.inner.pending.clone());
            }
            let r#gen = (dep.check_generation)();
            dep_pairs.push((dep.check_generation, r#gen));
        }

        *self.inner.deps.write().unwrap_or_else(|e| e.into_inner()) = dep_pairs;
    }

    /// 检查依赖是否有变化，如有则重新执行。
    /// 返回 `true` 表示重新执行了。
    pub fn tick(&self) -> bool {
        if !self.inner.pending.swap(false, Ordering::AcqRel) {
            return false;
        }

        let need_run = {
            let deps = self.inner.deps.read().unwrap_or_else(|e| e.into_inner());
            deps.iter()
                .any(|(check, cached_gen)| check() != *cached_gen)
        };

        if need_run {
            let (_, new_deps) = collect_deps(&self.inner.effect_fn);
            self.refresh_deps(new_deps);
            true
        } else {
            false
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════
// 将依赖追踪的 panic 生命周期测试置于独立文件，避免产品文件超过行数上限。
#[cfg(test)]
// 从仓库测试目录引入私有模块测试，使测试仍可访问本模块私有契约。
#[path = "../../../tests/unit/ui/reactive_state_tests.rs"]
// 将外置测试作为本响应式状态模块的私有子模块编译。
mod tests;
