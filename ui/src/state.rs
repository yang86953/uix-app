use std::cell::RefCell;
use std::fmt;
use std::sync::{Arc, RwLock};

// ── 响应式依赖追踪 ────────────────────────────────────────────
//
// 设计：使用 thread_local 追踪当前正在计算的 Computed 所读取的 State。
// State::get() 在追踪启用时自动注册依赖，Computed 在计算完毕后收集
// 这些依赖的 generation 快照，后续 get() 时比对以判断是否需要重新计算。

thread_local! {
    static TRACKING_DEPS: RefCell<Option<Vec<Box<dyn Fn() -> u64 + Send + Sync>>>> =
        RefCell::new(None);
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
pub struct State<T> {
    inner: Arc<RwLock<StateInner<T>>>,
}

struct StateInner<T> {
    value: T,
    generation: u64,
    #[allow(clippy::type_complexity)]
    watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>,
}

impl<T: Clone + Send + Sync + 'static> State<T> {
    pub fn new(value: T) -> Self {
        Self {
            inner: Arc::new(RwLock::new(StateInner {
                value,
                generation: 0,
                watchers: Vec::new(),
            })),
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
        }
    }

    pub fn get(&self) -> T {
        // 检查依赖是否变化
        let need_recompute = {
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
///
/// 创建时执行闭包，自动追踪其中读取的所有 State。
/// 当任意依赖的 generation 变化时，自动重新执行。
///
/// 适合替代手动 `watch()` 组合，用于日志、持久化、触发 UI 刷新等场景。
///
/// # 示例
/// ```ignore
/// let count = State::new(0);
/// let eff = Effect::new(|| {
///     println!("count = {}", count.get());
/// });
/// count.set(1); // 自动打印 "count = 1"
/// ```
pub struct Effect {
    effect_fn: Box<dyn Fn() + Send + Sync>,
    deps: Arc<RwLock<Vec<(Box<dyn Fn() -> u64 + Send + Sync>, u64)>>>,
}

impl Effect {
    pub fn new<F: Fn() + Send + Sync + 'static>(f: F) -> Self {
        // 首次运行收集依赖
        let (_, deps) = collect_deps(&f);
        let dep_pairs: Vec<_> = deps
            .into_iter()
            .map(|check| {
                let gen = check();
                (check, gen)
            })
            .collect();

        Self {
            effect_fn: Box::new(f),
            deps: Arc::new(RwLock::new(dep_pairs)),
        }
    }

    /// 检查依赖是否有变化，如有则重新执行。
    /// 返回 `true` 表示重新执行了。
    pub fn tick(&self) -> bool {
        let need_run = {
            let deps = self.deps.read().unwrap_or_else(|e| e.into_inner());
            deps.iter()
                .any(|(check, cached_gen)| check() != *cached_gen)
        };

        if need_run {
            let (_, new_deps) = collect_deps(&self.effect_fn);
            let new_pairs: Vec<_> = new_deps
                .into_iter()
                .map(|check| {
                    let gen = check();
                    (check, gen)
                })
                .collect();
            let mut deps = self.deps.write().unwrap_or_else(|e| e.into_inner());
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[test]
    fn state_get_set() {
        let s = State::new(42i32);
        assert_eq!(s.get(), 42);
        s.set(100);
        assert_eq!(s.get(), 100);
    }

    #[test]
    fn state_update() {
        let s = State::new(String::from("hello"));
        s.update(|v| v.push_str(" world"));
        assert_eq!(s.get(), "hello world");
    }

    #[test]
    fn state_generation() {
        let s = State::new(1i32);
        assert_eq!(s.generation(), 0);
        s.set(2);
        assert_eq!(s.generation(), 1);
        s.update(|v| *v += 1);
        assert_eq!(s.generation(), 2);
    }

    #[test]
    fn state_watch() {
        let s = State::new(0i32);
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        s.watch(move |v| {
            assert_eq!(*v, 42);
            called_clone.store(true, Ordering::SeqCst);
        });
        s.set(42);
        assert!(called.load(Ordering::SeqCst));
    }

    #[test]
    fn state_clone_shares_data() {
        let a = State::new(10i32);
        let b = a.clone();
        b.set(99);
        assert_eq!(a.get(), 99);
        assert_eq!(a.generation(), 1);
        assert_eq!(b.generation(), 1);
    }

    #[test]
    fn computed_auto_tracks_deps() {
        let a = State::new(1);
        let b = State::new(2);
        let a2 = a.clone();
        let b2 = b.clone();
        let sum = Computed::new(move || a2.get() + b2.get());
        assert_eq!(sum.get(), 3);

        a.set(10);
        assert_eq!(sum.get(), 12);

        b.set(20);
        assert_eq!(sum.get(), 30);
    }

    #[test]
    fn computed_does_not_recompute_when_deps_unchanged() {
        let a = State::new(42);
        let a2 = a.clone();
        let compute_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let cc = compute_count.clone();
        let c = Computed::new(move || {
            cc.fetch_add(1, Ordering::SeqCst);
            a2.get()
        });
        assert_eq!(c.get(), 42);
        assert_eq!(c.get(), 42);
        assert_eq!(compute_count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn computed_complex_deps() {
        let a = State::new(1);
        let b = State::new(2);
        let c = State::new(3);
        let a2 = a.clone();
        let b2 = b.clone();
        let c2 = c.clone();
        let sum = Computed::new(move || a2.get() + b2.get() + c2.get());
        assert_eq!(sum.get(), 6);

        a.set(10);
        assert_eq!(sum.get(), 15);

        c.set(30);
        assert_eq!(sum.get(), 42);
    }

    #[test]
    fn effect_auto_tracks() {
        let a = State::new(0);
        let a2 = a.clone();
        let last_val = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let lv = last_val.clone();
        let eff = Effect::new(move || {
            lv.store(a2.get(), Ordering::SeqCst);
        });
        assert!(!eff.tick());
        a.set(42);
        assert!(eff.tick());
        assert_eq!(last_val.load(Ordering::SeqCst), 42);
        assert!(!eff.tick());
    }

    #[test]
    fn computed_basic() {
        let c = Computed::new(|| 42i32);
        assert_eq!(c.get(), 42);
    }

    #[test]
    fn computed_invalidate() {
        let cell = Arc::new(std::sync::atomic::AtomicI32::new(0));
        let cell2 = Arc::clone(&cell);
        let c = Computed::new(move || cell2.load(Ordering::SeqCst));
        assert_eq!(c.get(), 0);
        cell.store(5, Ordering::SeqCst);
        // 此时 Computed 的依赖（构造时的 State）未变，但 cell 变了
        // 需要用 invalidate 强制刷新（当依赖无法自动追踪时）
        c.invalidate();
        assert_eq!(c.get(), 5);
    }
}
