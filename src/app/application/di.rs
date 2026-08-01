use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Simple DI container for service registration and resolution.
/// 所有 singleton 以 `Arc` 共享，clone 只复制 Arc 指针，不深拷贝值。
/// 跨 AppHandle / 多窗修改同一服务时共享同一实例。
#[derive(Default, Clone)]
pub struct Container {
    singletons: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl Container {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a singleton instance (wrapped in Arc for shared ownership).
    pub fn singleton<T: Any + Send + Sync>(&mut self, instance: T) {
        self.singletons
            .insert(TypeId::of::<T>(), Arc::new(instance));
    }

    /// 批量注册服务对象（E-08）：聚合结构体整体注册为单例；
    /// 组件经 `resolve::<AppServices>()` 取用，不各自 new 全局服务。
    pub fn register<T: Any + Send + Sync>(&mut self, instance: T) {
        self.singleton(instance);
    }

    /// Resolve a service by type.
    pub fn resolve<T: Any + Send + Sync>(&self) -> Option<&T> {
        self.singletons
            .get(&TypeId::of::<T>())
            .and_then(|arc| arc.downcast_ref::<T>())
    }

    /// Resolve a clone of a singleton for runtime handles.
    pub fn resolve_clone<T: Any + Send + Sync + Clone>(&self) -> Option<T> {
        self.resolve::<T>().cloned()
    }

    pub fn has<T: Any + Send + Sync>(&self) -> bool {
        self.singletons.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: Any + Send + Sync>(&mut self) {
        self.singletons.remove(&TypeId::of::<T>());
    }
}
