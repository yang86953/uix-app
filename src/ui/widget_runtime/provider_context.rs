//! Type-indexed context snapshots owned and propagated by the view runtime.
use std::{
    any::{Any, TypeId},
    cell::RefCell,
    collections::HashMap,
    ptr::NonNull,
    sync::{Arc, OnceLock},
};
trait Entry: Any + Send + Sync {
    fn as_any(&self) -> &dyn Any;
    fn equal(&self, other: &dyn Entry) -> bool;
}
impl<T: Any + Send + Sync + PartialEq> Entry for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn equal(&self, other: &dyn Entry) -> bool {
        other.as_any().downcast_ref::<T>() == Some(self)
    }
}
#[derive(Clone)]
pub struct ProviderContext {
    values: Arc<HashMap<TypeId, Arc<dyn Entry>>>,
}
impl Default for ProviderContext {
    fn default() -> Self {
        static EMPTY: OnceLock<Arc<HashMap<TypeId, Arc<dyn Entry>>>> = OnceLock::new();
        Self {
            values: EMPTY.get_or_init(Default::default).clone(),
        }
    }
}
impl PartialEq for ProviderContext {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.values, &other.values)
            || (self.values.len() == other.values.len()
                && self.values.iter().all(|(key, value)| {
                    other
                        .values
                        .get(key)
                        .is_some_and(|v| value.equal(v.as_ref()))
                }))
    }
}
impl ProviderContext {
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.values.get(&TypeId::of::<T>())?.as_any().downcast_ref()
    }
    pub fn with<T: Clone + PartialEq + Send + Sync + 'static>(&self, value: T) -> Self {
        let mut next = self.clone();
        if self.get::<T>() != Some(&value) {
            Arc::make_mut(&mut next.values).insert(TypeId::of::<T>(), Arc::new(value));
        }
        next
    }
    pub(crate) fn style_scope(&self) -> super::config::StyleScope {
        self.get().cloned().unwrap_or_default()
    }
}
/// Reads the nearest value of this type, falling back to its own default.
pub fn use_context<T: Clone + Default + 'static>() -> T {
    current_provider_context()
        .get()
        .cloned()
        .unwrap_or_default()
}
/// Installs an immutable value for synchronous view construction; nested scopes and unwinding restore the caller.
pub fn with_context<T: Clone + PartialEq + Send + Sync + 'static, R>(
    value: &T,
    f: impl FnOnce() -> R,
) -> R {
    with_provider_context(&current_provider_context().with(value.clone()), f)
}
thread_local! {
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer already uses an inline const block; Clippy reports the macro expansion"
    )]
    static PROVIDER_CONTEXT_STACK: RefCell<Vec<ProviderContextFrame>> = const { RefCell::new(Vec::new()) };
}

/// 同步闭包有效期内借用 ProviderContext，不参与共享快照的引用计数。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct ProviderContextFrame(NonNull<ProviderContext>);

impl ProviderContextFrame {
    fn clone_context(self) -> ProviderContext {
        // SAFETY: frame 只由 `with_provider_context` 从活跃引用创建，且守卫在
        // 该同步闭包返回或展开前必定弹出 frame，因此读取期间来源仍然存活。
        unsafe { self.0.as_ref().clone() }
    }
}

struct ProviderContextGuard {
    frame: ProviderContextFrame,
}

impl Drop for ProviderContextGuard {
    fn drop(&mut self) {
        PROVIDER_CONTEXT_STACK.with(|stack| {
            let popped = stack.borrow_mut().pop();
            debug_assert_eq!(popped, Some(self.frame));
        });
    }
}

pub fn current_provider_context() -> ProviderContext {
    PROVIDER_CONTEXT_STACK.with(|stack| {
        stack
            .borrow()
            .last()
            .copied()
            .map(ProviderContextFrame::clone_context)
            .unwrap_or_default()
    })
}

pub fn with_provider_context<T>(context: &ProviderContext, f: impl FnOnce() -> T) -> T {
    let frame = ProviderContextFrame(NonNull::from(context));
    PROVIDER_CONTEXT_STACK.with(|stack| stack.borrow_mut().push(frame));
    let _guard = ProviderContextGuard { frame };
    f()
}
