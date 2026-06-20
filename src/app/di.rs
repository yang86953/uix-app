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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_container_is_empty() {
        let c = Container::new();
        assert!(!c.has::<String>());
        assert!(!c.has::<i32>());
    }

    #[test]
    fn singleton_register_and_resolve() {
        let mut c = Container::new();
        c.singleton("hello".to_string());
        c.singleton(42i32);

        assert!(c.has::<String>());
        assert!(c.has::<i32>());

        assert_eq!(c.resolve::<String>(), Some(&"hello".to_string()));
        assert_eq!(c.resolve::<i32>(), Some(&42i32));
    }

    #[test]
    fn resolve_mut_allows_modification() {
        let mut c = Container::new();
        c.singleton(10i32);

        let val = c.resolve_mut::<i32>();
        assert!(val.is_some());
        *val.unwrap() = 20;

        assert_eq!(c.resolve::<i32>(), Some(&20i32));
    }

    #[test]
    fn resolve_nonexistent_returns_none() {
        let c = Container::new();
        assert!(c.resolve::<String>().is_none());
        assert!(c.resolve::<f64>().is_none());
    }

    #[test]
    fn remove_clears_singleton() {
        let mut c = Container::new();
        c.singleton(42i32);
        assert!(c.has::<i32>());
        c.remove::<i32>();
        assert!(!c.has::<i32>());
    }

    #[test]
    fn multiple_singletons_independent() {
        let mut c = Container::new();
        c.singleton(1i32);
        c.singleton(2.0f64);
        c.singleton("text".to_string());

        assert_eq!(c.resolve::<i32>(), Some(&1i32));
        assert_eq!(c.resolve::<f64>(), Some(&2.0f64));
        assert_eq!(c.resolve::<String>(), Some(&"text".to_string()));

        c.remove::<i32>();
        assert!(c.resolve::<i32>().is_none());
        assert!(c.resolve::<f64>().is_some());
        assert!(c.resolve::<String>().is_some());
    }

    #[test]
    fn singleton_clone_trait() {
        let mut c = Container::new();
        c.singleton(vec![1, 2, 3]);
        assert_eq!(c.resolve::<Vec<i32>>(), Some(&vec![1, 2, 3]));
    }

    #[test]
    fn default_container_is_empty() {
        let c: Container = Default::default();
        assert!(!c.has::<i32>());
    }
}
