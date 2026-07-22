use std::any::TypeId;
use std::cell::{Cell, RefCell};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashSet;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use crate::core::{ComponentId, Rect};
use crate::draw::pipeline::{invalidate_paint_handle, InvalidationQueueHandle};

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

// Phase 6：State 读取时暂存，供 DynamicLabel 等响应式 widget 绑定。
thread_local! {
    static PENDING_STATE_BINDS: RefCell<Vec<(StateSlotId, Arc<dyn StatePaintBind>)>> =
        const { RefCell::new(Vec::new()) };
}

// layout 后探测 DynamicLabel 闭包时捕获 `State::get()` 读取的实例。
thread_local! {
    static STATE_BIND_CAPTURE: RefCell<Option<StateBindCapture>> = const { RefCell::new(None) };
}

// View 构建期暂存的 Effect（build 后注册到 WidgetTree）。
thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static PENDING_EFFECTS: RefCell<Vec<Effect>> = const { RefCell::new(Vec::new()) };
}

// 须为 thread_local：并行测试/多窗口同时 capture View 时，全局标志会导致
// 先结束的 capture 关闭捕获，使同线程其他 capture 中的 State::get 无法登记 pending。
thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static STATE_CAPTURE_ACTIVE: Cell<bool> = const { Cell::new(false) };
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
pub fn begin_state_capture() {
    STATE_CAPTURE_ACTIVE.with(|active| active.set(true));
    PENDING_STATE_BINDS.with(|p| p.borrow_mut().clear());
    PENDING_EFFECTS.with(|p| p.borrow_mut().clear());
}

/// 结束 View 构建期的 State 捕获。
pub fn end_state_capture() {
    STATE_CAPTURE_ACTIVE.with(|active| active.set(false));
}

fn state_capture_active() -> bool {
    STATE_CAPTURE_ACTIVE.with(Cell::get)
}

/// 取出并清空未关联 widget 的 pending State 绑定（build 末兜底）。
pub fn drain_pending_state_binds() -> Vec<Arc<dyn StatePaintBind>> {
    PENDING_STATE_BINDS.with(|p| {
        std::mem::take(&mut *p.borrow_mut())
            .into_iter()
            .map(|(_, bind)| bind)
            .collect()
    })
}

/// 取出 View 构建期捕获的 Effect。
pub fn drain_pending_effects() -> Vec<Effect> {
    PENDING_EFFECTS.with(|p| std::mem::take(&mut *p.borrow_mut()))
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
            crate::core::log::warn_fn(format_args!(
                "State 绑定探测 component_id 不一致: 期望 {component_id}, 实际 {id}"
            ));
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
    if !STATE_CAPTURE_ACTIVE.with(|active| active.get()) {
        return;
    }
    let slot_id = state.slot_id();
    PENDING_STATE_BINDS.with(|p| {
        let mut pending = p.borrow_mut();
        if pending
            .iter()
            .any(|(captured_slot, _)| *captured_slot == slot_id)
        {
            return;
        }
        let bind: Arc<dyn StatePaintBind> = Arc::new(state.clone());
        pending.push((slot_id, bind));
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
            site.callback = callback;
        } else {
            guard.push(ReconcileBindSite { key, callback });
        }
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

/// 在当前线程启用依赖追踪，执行闭包后返回收集到的依赖 generation 检查器列表。
/// 支持嵌套：内层 collect_deps 保存并恢复外层追踪上下文，使 `Computed` 在其 get()
/// 内部也能被外层正确追踪。
fn collect_deps<F, R>(f: F) -> (R, Vec<EffectDependency>)
where
    F: FnOnce() -> R,
{
    TRACKING_DEPS.with(|deps| {
        let outer = deps.borrow_mut().take();
        *deps.borrow_mut() = Some(Vec::new());
        let result = f();
        let collected = deps.borrow_mut().take().unwrap_or_default();
        *deps.borrow_mut() = outer;
        (result, collected)
    })
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

    /// 设置 reconcile invalidation 回调。此回调在值变更时（`set` / `update`）自动调用。
    /// 由 ViewAdapter 内部使用，用户不需要调用此方法。
    pub fn set_reconcile_invalidation_fn<F: Fn() + Send + Sync + 'static>(&self, f: F) {
        if let Ok(mut guard) = self.reconcile_sites.lock() {
            guard.clear();
            guard.push(ReconcileBindSite {
                key: 0,
                callback: Arc::new(f),
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
                let gen = (dep.check_generation)();
                (dep.check_generation, gen)
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
                    let gen = (dep.check_generation)();
                    (dep.check_generation, gen)
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
                let gen = (dep.check_generation)();
                (dep.check_generation, gen)
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
        if STATE_CAPTURE_ACTIVE.with(|active| active.get()) {
            PENDING_EFFECTS.with(|p| p.borrow_mut().push(effect.clone()));
        }
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
            let gen = (dep.check_generation)();
            dep_pairs.push((dep.check_generation, gen));
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
