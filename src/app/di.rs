use std::any::{Any, TypeId};
use std::collections::HashMap;

/// Simple DI container for service registration and resolution.
#[derive(Default)]
pub struct Container {
    singletons: HashMap<TypeId, Box<dyn Any + Send>>,
}

impl Container {
    pub fn new() -> Self { Self::default() }

    /// Register a singleton instance.
    pub fn singleton<T: Any + Send + Clone>(&mut self, instance: T) {
        self.singletons.insert(TypeId::of::<T>(), Box::new(instance));
    }

    /// Resolve a service by type.
    pub fn resolve<T: Any + Send>(&self) -> Option<&T> {
        if let Some(singleton) = self.singletons.get(&TypeId::of::<T>()) {
            return singleton.downcast_ref::<T>();
        }
        None
    }

    /// Resolve a mutable service by type.
    pub fn resolve_mut<T: Any + Send>(&mut self) -> Option<&mut T> {
        if let Some(singleton) = self.singletons.get_mut(&TypeId::of::<T>()) {
            return singleton.downcast_mut::<T>();
        }
        None
    }

    pub fn has<T: Any + Send>(&self) -> bool {
        self.singletons.contains_key(&TypeId::of::<T>())
    }

    pub fn remove<T: Any + Send>(&mut self) {
        self.singletons.remove(&TypeId::of::<T>());
    }
}
