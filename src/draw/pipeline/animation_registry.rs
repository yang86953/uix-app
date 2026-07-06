//! Animation registry for nodes driven by the centralized animation system.

use std::collections::HashSet;

use super::invalidation::NodeId;

/// Tracks active animation nodes without walking the whole widget tree.
#[derive(Debug, Clone, Default)]
pub struct AnimationRegistry {
    active: HashSet<NodeId>,
}

impl AnimationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a node with active centralized animation work.
    pub fn register(&mut self, id: NodeId) {
        self.active.insert(id);
    }

    pub fn unregister(&mut self, id: NodeId) {
        self.active.remove(&id);
    }

    pub fn is_registered(&self, id: NodeId) -> bool {
        self.active.contains(&id)
    }

    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    pub fn active_ids(&self) -> Vec<NodeId> {
        self.active.iter().copied().collect()
    }

    pub fn clear(&mut self) {
        self.active.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_unregister_lifecycle() {
        let mut reg = AnimationRegistry::new();
        assert!(!reg.has_active());
        reg.register(3);
        assert!(reg.has_active());
        assert!(reg.is_registered(3));
        reg.unregister(3);
        assert!(!reg.has_active());
    }

    #[test]
    fn active_ids_snapshot() {
        let mut reg = AnimationRegistry::new();
        reg.register(1);
        reg.register(5);
        let mut ids = reg.active_ids();
        ids.sort_unstable();
        assert_eq!(ids, vec![1, 5]);
    }
}
