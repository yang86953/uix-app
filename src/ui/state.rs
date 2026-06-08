use std::fmt;
use std::sync::{Arc, RwLock};

/// A reactive state value that notifies watchers on change.
/// Thread-safe: Send + Sync when T is Send + Sync.
pub struct State<T> {
    inner: Arc<RwLock<StateInner<T>>>,
}

struct StateInner<T> {
    value: T,
    generation: u64,
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
        self.inner
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .value
            .clone()
    }

    pub fn set(&self, value: T) {
        let watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>;
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            inner.value = value;
            inner.generation += 1;
            // Clone Arcs before releasing the lock to avoid unsafe pointer dereference.
            watchers = inner.watchers.iter().map(|w| Arc::clone(w)).collect();
        }
        // Notify watchers outside the lock to prevent deadlocks
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        for watcher in &watchers {
            watcher(&inner.value);
        }
    }

    pub fn update<F>(&self, f: F)
    where
        F: FnOnce(&mut T),
    {
        let watchers: Vec<Arc<dyn Fn(&T) + Send + Sync>>;
        {
            let mut inner = self.inner.write().unwrap_or_else(|e| e.into_inner());
            f(&mut inner.value);
            inner.generation += 1;
            // Clone Arcs before releasing the lock to avoid unsafe pointer dereference.
            watchers = inner.watchers.iter().map(|w| Arc::clone(w)).collect();
        }
        let inner = self.inner.read().unwrap_or_else(|e| e.into_inner());
        for watcher in &watchers {
            watcher(&inner.value);
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

/// A computed state derived from other states.
/// Thread-safe: Send + Sync when T is Send + Sync.
pub struct Computed<T> {
    compute_fn: Box<dyn Fn() -> T + Send + Sync>,
    cached: Arc<RwLock<Option<T>>>,
}

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    pub fn new<F: Fn() -> T + Send + Sync + 'static>(f: F) -> Self {
        Self {
            compute_fn: Box::new(f),
            cached: Arc::new(RwLock::new(None)),
        }
    }

    pub fn get(&self) -> T {
        let mut cached = self.cached.write().unwrap_or_else(|e| e.into_inner());
        match cached.as_ref() {
            Some(v) => v.clone(),
            None => {
                let v = (self.compute_fn)();
                *cached = Some(v.clone());
                v
            }
        }
    }

    pub fn invalidate(&self) {
        *self.cached.write().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

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
        let called = std::sync::Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        s.watch(move |v| {
            assert_eq!(*v, 42);
            called_clone.store(true, Ordering::SeqCst);
        });
        s.set(42);
        // Watcher is called synchronously within set()
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
    fn computed_basic() {
        let c = Computed::new(|| 42i32);
        assert_eq!(c.get(), 42);
    }

    #[test]
    fn computed_invalidate() {
        use std::sync::atomic::{AtomicI32, Ordering};
        use std::sync::Arc;
        let cell = Arc::new(AtomicI32::new(0));
        let cell2 = Arc::clone(&cell);
        let c = Computed::new(move || cell2.load(Ordering::SeqCst));
        // First get() caches the closure result (0)
        assert_eq!(c.get(), 0);
        // Mutate the underlying data
        cell.store(5, Ordering::SeqCst);
        // Still cached
        assert_eq!(c.get(), 0);
        // Invalidate and re-compute
        c.invalidate();
        assert_eq!(c.get(), 5);
    }
}
