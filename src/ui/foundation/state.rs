use std::cell::RefCell;
use std::fmt;
use std::sync::{Arc, RwLock};

use crate::draw::pipeline::{invalidate_paint_handle, InvalidationQueueHandle};
use crate::native::Rect;

// ── 响应式依赖追踪 ────────────────────────────────────────────
//
// 设计：使用 thread_local 追踪当前正在计算的 Computed 所读取的 State。
// State::get() 在追踪启用时自动注册依赖，Computed 在计算完毕后收集
// 这些依赖的 generation 快照，后续 get() 时比对以判断是否需要重新计算。

thread_local! {
    static TRACKING_DEPS: RefCell<Option<Vec<Box<dyn Fn() -> u64 + Send + Sync>>>> =
        RefCell::new(None);
}

// 当前 View 的脏标记回调——State::new 创建时自动读取并绑定。
thread_local! {
    static CURRENT_VIEW_DIRTY_FN: RefCell<Option<Arc<dyn Fn() + Send + Sync>>> =
        RefCell::new(None);
}

// Phase 6：State 创建时暂存，供 DynamicLabel 等响应式 widget 绑定。
thread_local! {
    static PENDING_STATE_BINDS: RefCell<Vec<Arc<dyn StatePaintBind>>> = RefCell::new(Vec::new());
}

// layout 后探测 DynamicLabel 闭包时捕获 `State::get()` 读取的实例。
thread_local! {
    static STATE_BIND_CAPTURE: RefCell<
        Option<(usize, InvalidationQueueHandle, Option<Rect>, Vec<Arc<dyn StatePaintBind>>)>,
    > = RefCell::new(None);
}

// View 构建期暂存的 Effect（build 后注册到 WidgetTree）。
thread_local! {
    static PENDING_EFFECTS: RefCell<Vec<Effect>> = RefCell::new(Vec::new());
}

static STATE_CAPTURE_ACTIVE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// 开始捕获 `State::new` / `Effect::new` 实例（View 构建期间调用）。
pub fn begin_state_capture() {
    STATE_CAPTURE_ACTIVE.store(true, std::sync::atomic::Ordering::SeqCst);
    PENDING_STATE_BINDS.with(|p| p.borrow_mut().clear());
    PENDING_EFFECTS.with(|p| p.borrow_mut().clear());
}

/// 结束 View 构建期的 State 捕获。
pub fn end_state_capture() {
    STATE_CAPTURE_ACTIVE.store(false, std::sync::atomic::Ordering::SeqCst);
}

/// 取出并清空未关联 widget 的 pending State 绑定（build 末兜底）。
pub fn drain_pending_state_binds() -> Vec<Arc<dyn StatePaintBind>> {
    PENDING_STATE_BINDS.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

/// 取出 View 构建期捕获的 Effect。
pub fn drain_pending_effects() -> Vec<Effect> {
    PENDING_EFFECTS.with(|p| std::mem::take(&mut *p.borrow_mut()))
}

/// 开始探测 DynamicLabel 闭包内读取的 State / Computed（layout 后 bind 阶段调用）。
pub fn begin_state_bind_capture(
    widget_id: usize,
    queue: InvalidationQueueHandle,
    rect: Option<Rect>,
) {
    STATE_BIND_CAPTURE.with(|c| {
        *c.borrow_mut() = Some((widget_id, queue, rect, Vec::new()));
    });
}

/// 结束探测并将捕获到的 State 绑定到指定 widget。
pub fn end_state_bind_capture(widget_id: usize) {
    STATE_BIND_CAPTURE.with(|c| {
        let Some((id, queue, rect, states)) = c.borrow_mut().take() else {
            return;
        };
        if id != widget_id {
            crate::core::log::warn_fn(format!(
                "State 绑定探测 widget_id 不一致: 期望 {widget_id}, 实际 {id}"
            ));
        }
        for source in states {
            source.bind_paint(widget_id, queue.clone(), rect);
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

fn try_capture_computed_bind<T: Clone + Send + Sync + 'static>(computed: &Computed<T>) {
    STATE_BIND_CAPTURE.with(|c| {
        if let Some((widget_id, queue, rect, _)) = c.borrow().as_ref() {
            computed.bind_paint_invalidation(*widget_id, queue.clone(), *rect);
        }
    });
}

fn fire_paint_binding(
    binding: &Arc<std::sync::Mutex<Option<(usize, InvalidationQueueHandle, Option<Rect>)>>>,
) {
    if let Ok(guard) = binding.lock() {
        if let Some((id, q, r)) = guard.as_ref() {
            invalidate_paint_handle(q, *id, *r);
        }
    }
}

/// State 变更时推送精确 Paint 失效的绑定接口。
pub trait StatePaintBind: Send + Sync {
    fn bind_paint(&self, widget_id: usize, queue: InvalidationQueueHandle, rect: Option<Rect>);
}

impl<T: Clone + Send + Sync + 'static> StatePaintBind for State<T> {
    fn bind_paint(&self, widget_id: usize, queue: InvalidationQueueHandle, rect: Option<Rect>) {
        self.bind_paint_invalidation(widget_id, queue, rect);
    }
}

impl<T: Clone + Send + Sync + 'static> StatePaintBind for Computed<T> {
    fn bind_paint(&self, widget_id: usize, queue: InvalidationQueueHandle, rect: Option<Rect>) {
        self.bind_paint_invalidation(widget_id, queue, rect);
    }
}

/// 设置当前 View 的脏标记回调。此回调会被新创建的 `State` 自动绑定。
/// 由 ViewAdapter 内部调用，用户不需要直接使用。
pub fn set_current_view_dirty_fn<F: Fn() + Send + Sync + 'static>(f: F) {
    CURRENT_VIEW_DIRTY_FN.with(|dirty| {
        *dirty.borrow_mut() = Some(Arc::new(f));
    });
}

/// 清除当前 View 的脏标记回调。
pub fn clear_current_view_dirty_fn() {
    CURRENT_VIEW_DIRTY_FN.with(|dirty| {
        *dirty.borrow_mut() = None;
    });
}

/// 在当前线程启用依赖追踪，执行闭包后返回收集到的依赖 generation 检查器列表。
fn collect_deps<F, R>(f: F) -> (R, Vec<Box<dyn Fn() -> u64 + Send + Sync>>)
where
    F: FnOnce() -> R,
{
    TRACKING_DEPS.with(|deps| {
        *deps.borrow_mut() = Some(Vec::new());
    });
    let result = f();
    let collected = TRACKING_DEPS.with(|deps| deps.borrow_mut().take().unwrap_or_default());
    (result, collected)
}

/// 将当前 State 注册到追踪上下文中（如果追踪已启用）。
fn track_dep<F>(register: F)
where
    F: FnOnce() -> Box<dyn Fn() -> u64 + Send + Sync>,
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
/// 支持自动脏标记：当通过 `set()` / `update()` 修改值时，自动调用注册的脏标记回调，
/// 通知 WidgetTree 重新渲染所属 View。脏标记回调由 ViewAdapter 在 ViewNode 展开时自动绑定，
/// 用户不需要手动调用 `mark_dirty`。
pub struct State<T> {
    inner: Arc<RwLock<StateInner<T>>>,
    /// 脏标记回调——值变更时自动调用，通知 WidgetTree 重绘所属节点。
    dirty_fn: Arc<std::sync::Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
    /// Phase 6：精确 Paint 失效绑定（WidgetId + 队列句柄）。
    paint_binding: Arc<std::sync::Mutex<Option<(usize, InvalidationQueueHandle, Option<Rect>)>>>,
}

struct StateInner<T> {
    value: T,
    generation: u64,
    #[allow(clippy::type_complexity)]
    watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>,
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    pub fn new(value: T) -> Self {
        // 自动从线程局部上下文绑定脏标记回调
        let dirty_fn: Arc<std::sync::Mutex<Option<Box<dyn Fn() + Send + Sync>>>> =
            Arc::new(std::sync::Mutex::new(None));
        let paint_binding = Arc::new(std::sync::Mutex::new(None));
        CURRENT_VIEW_DIRTY_FN.with(|dirty| {
            let borrowed = dirty.borrow();
            if let Some(ref f) = *borrowed {
                let cb = f.clone();
                if let Ok(mut guard) = dirty_fn.lock() {
                    *guard = Some(Box::new(move || cb()));
                }
            }
        });
        let state = Self {
            inner: Arc::new(RwLock::new(StateInner {
                value,
                generation: 0,
                watchers: Vec::new(),
            })),
            dirty_fn,
            paint_binding,
        };
        if STATE_CAPTURE_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
            let bind: Arc<dyn StatePaintBind> = Arc::new(state.clone());
            PENDING_STATE_BINDS.with(|p| p.borrow_mut().push(bind));
        }
        state
    }

    /// 绑定精确 Paint 失效：State 变更时向队列推送 `Invalidation::Paint`。
    pub fn bind_paint_invalidation(
        &self,
        widget_id: usize,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        if let Ok(mut guard) = self.paint_binding.lock() {
            *guard = Some((widget_id, queue.clone(), rect));
        }
        let paint_binding = self.paint_binding.clone();
        if let Ok(mut guard) = self.dirty_fn.lock() {
            *guard = Some(Box::new(move || {
                if let Ok(binding) = paint_binding.lock() {
                    if let Some((id, q, r)) = binding.as_ref() {
                        invalidate_paint_handle(q, *id, *r);
                    }
                }
            }));
        }
    }

    /// 设置脏标记回调。此回调在值变更时（`set` / `update`）自动调用。
    /// 由 ViewAdapter 内部使用，用户不需要调用此方法。
    pub fn set_dirty_fn<F: Fn() + Send + Sync + 'static>(&self, f: F) {
        if let Ok(mut guard) = self.dirty_fn.lock() {
            *guard = Some(Box::new(f));
        }
    }

    pub fn get(&self) -> T {
        // 将自身注册到活跃的追踪上下文中（如 Computed 计算期间）
        let self_clone = self.inner.clone();
        track_dep(move || {
            Box::new(move || {
                self_clone
                    .read()
                    .unwrap_or_else(|e| e.into_inner())
                    .generation
            })
        });
        try_capture_state_bind(self);

        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .value
            .clone()
    }

    pub fn set(&self, value: T) {
        let watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>;
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
        Self::fire_invalidation(&self.dirty_fn, &self.paint_binding);
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>;
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
        Self::fire_invalidation(&self.dirty_fn, &self.paint_binding);
    }

    fn fire_invalidation(
        dirty_fn: &Arc<std::sync::Mutex<Option<Box<dyn Fn() + Send + Sync>>>>,
        paint_binding: &Arc<
            std::sync::Mutex<Option<(usize, InvalidationQueueHandle, Option<Rect>)>>,
        >,
    ) {
        let has_paint_binding = paint_binding
            .lock()
            .ok()
            .is_some_and(|guard| guard.is_some());
        if has_paint_binding {
            fire_paint_binding(paint_binding);
            return;
        }
        if let Ok(guard) = dirty_fn.lock() {
            if let Some(ref f) = *guard {
                f();
            }
        }
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
}

impl<T: Clone + Send + Sync + 'static> Clone for State<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            dirty_fn: self.dirty_fn.clone(),
            paint_binding: self.paint_binding.clone(),
        }
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for State<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
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
    compute_fn: Box<dyn Fn() -> T + Send + Sync>,
    cached: Arc<RwLock<Option<T>>>,
    /// 依赖的 generation 检查器列表：(检查器, 上次计算时的 generation)
    deps: Arc<RwLock<Vec<(Box<dyn Fn() -> u64 + Send + Sync>, u64)>>>,
    /// Phase R2：精确 Paint 失效绑定。
    paint_binding: Arc<std::sync::Mutex<Option<(usize, InvalidationQueueHandle, Option<Rect>)>>>,
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    /// 创建一个自动追踪依赖的 Computed。
    ///
    /// 构造时会立即执行一次 `f` 以收集依赖，之后 `get()` 自动判断是否需要重算。
    pub fn new<F: Fn() -> T + Send + Sync + 'static>(f: F) -> Self {
        let (initial, deps) = collect_deps(&f);
        let dep_pairs: Vec<_> = deps
            .into_iter()
            .map(|check| {
                let gen = check();
                (check, gen)
            })
            .collect();

        Self {
            compute_fn: Box::new(f),
            cached: Arc::new(RwLock::new(Some(initial))),
            deps: Arc::new(RwLock::new(dep_pairs)),
            paint_binding: Arc::new(std::sync::Mutex::new(None)),
        }
    }

    /// 绑定精确 Paint 失效：依赖变化导致重算时向队列推送 `Invalidation::Paint`。
    pub fn bind_paint_invalidation(
        &self,
        widget_id: usize,
        queue: InvalidationQueueHandle,
        rect: Option<Rect>,
    ) {
        if let Ok(mut guard) = self.paint_binding.lock() {
            *guard = Some((widget_id, queue, rect));
        }
    }

    pub fn get(&self) -> T {
        try_capture_computed_bind(self);

        let force_probe = STATE_BIND_CAPTURE.with(|c| c.borrow().is_some());

        // 检查依赖是否变化（layout 探测阶段强制执行一次以捕获 State 绑定）
        let need_recompute = if force_probe {
            true
        } else {
            let deps = self.deps.read().unwrap_or_else(|e| e.into_inner());
            deps.iter()
                .any(|(check, cached_gen)| check() != *cached_gen)
        };

        if need_recompute {
            let (value, new_deps) = collect_deps(&self.compute_fn);
            let new_pairs: Vec<_> = new_deps
                .into_iter()
                .map(|check| {
                    let gen = check();
                    (check, gen)
                })
                .collect();

            let mut cached = self.cached.write().unwrap_or_else(|e| e.into_inner());
            *cached = Some(value.clone());
            let mut deps = self.deps.write().unwrap_or_else(|e| e.into_inner());
            *deps = new_pairs;
            fire_paint_binding(&self.paint_binding);
            value
        } else {
            let cached = self.cached.read().unwrap_or_else(|e| e.into_inner());
            cached.clone().unwrap_or_else(|| (self.compute_fn)())
        }
    }

    /// 强制使缓存失效并重新计算（当依赖无法被自动追踪时使用）。
    pub fn invalidate(&self) {
        let (value, _new_deps) = collect_deps(&self.compute_fn);
        *self.cached.write().unwrap_or_else(|e| e.into_inner()) = Some(value);
    }
}

impl<T: fmt::Debug + Clone + Send + Sync + 'static> fmt::Debug for Computed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Computed")
            .field("value", &self.get())
            .finish()
    }
}

/// ── Effect — 自动追踪依赖的副作用 ──────────────────────────────
struct EffectInner {
    effect_fn: Box<dyn Fn() + Send + Sync>,
    deps: RwLock<Vec<(Box<dyn Fn() -> u64 + Send + Sync>, u64)>>,
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
        let dep_pairs: Vec<_> = deps
            .into_iter()
            .map(|check| {
                let gen = check();
                (check, gen)
            })
            .collect();

        let effect = Self {
            inner: Arc::new(EffectInner {
                effect_fn: Box::new(f),
                deps: RwLock::new(dep_pairs),
            }),
        };
        if STATE_CAPTURE_ACTIVE.load(std::sync::atomic::Ordering::SeqCst) {
            PENDING_EFFECTS.with(|p| p.borrow_mut().push(effect.clone()));
        }
        effect
    }

    /// 检查依赖是否有变化，如有则重新执行。
    /// 返回 `true` 表示重新执行了。
    pub fn tick(&self) -> bool {
        let need_run = {
            let deps = self.inner.deps.read().unwrap_or_else(|e| e.into_inner());
            deps.iter()
                .any(|(check, cached_gen)| check() != *cached_gen)
        };

        if need_run {
            let (_, new_deps) = collect_deps(&self.inner.effect_fn);
            let new_pairs: Vec<_> = new_deps
                .into_iter()
                .map(|check| {
                    let gen = check();
                    (check, gen)
                })
                .collect();
            let mut deps = self.inner.deps.write().unwrap_or_else(|e| e.into_inner());
            *deps = new_pairs;
            true
        } else {
            false
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 测试
// ════════════════════════════════════════════════════════════════════════════
