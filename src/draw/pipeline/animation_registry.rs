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
#[path = "../../tests/draw/pipeline/animation_registry.rs"]
mod tests;
