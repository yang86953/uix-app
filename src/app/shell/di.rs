use std::any::{Any, TypeId};
use std::collections::HashMap;

trait ServiceEntry: Send {
    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn clone_entry(&self) -> Box<dyn ServiceEntry>;
}

impl<T> ServiceEntry for T
where
    T: Any + Send + Clone,
{
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    fn clone_entry(&self) -> Box<dyn ServiceEntry> {
        Box::new(self.clone())
    }
}

/// Simple DI container for service registration and resolution.
#[derive(Default)]
pub struct Container {
    singletons: HashMap<TypeId, Box<dyn ServiceEntry>>,
}

impl Clone for Container {
    fn clone(&self) -> Self {
        Self {
            singletons: self
                .singletons
                .iter()
                .map(|(&type_id, entry)| (type_id, entry.clone_entry()))
                .collect(),
        }
    }
}

impl Container {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a singleton instance.
    pub fn singleton<T: Any + Send + Clone>(&mut self, instance: T) {
        self.singletons
            .insert(TypeId::of::<T>(), Box::new(instance));
    }

    /// Resolve a service by type.
    pub fn resolve<T: Any + Send>(&self) -> Option<&T> {
        if let Some(singleton) = self.singletons.get(&TypeId::of::<T>()) {
            return singleton.as_any().downcast_ref::<T>();
        }
        None
    }

    /// Resolve a clone of a singleton for runtime handles.
    pub fn resolve_clone<T: Any + Send + Clone>(&self) -> Option<T> {
        self.resolve::<T>().cloned()
    }

    /// Resolve a mutable service by type.
    pub fn resolve_mut<T: Any + Send>(&mut self) -> Option<&mut T> {
        if let Some(singleton) = self.singletons.get_mut(&TypeId::of::<T>()) {
            return singleton.as_any_mut().downcast_mut::<T>();
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
