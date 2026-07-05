//! 动画注册表 — 仅 tick 需要持续更新的节点（Phase 2）。

use std::collections::HashSet;

use super::invalidation::NodeId;

/// 跟踪 `needs_continuous_update` 节点，避免全树 `traverse()`。
#[derive(Debug, Clone, Default)]
pub struct AnimationRegistry {
    active: HashSet<NodeId>,
}

impl AnimationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 注册需要每帧 `on_update` 的节点。
    pub fn register(&mut self, id: NodeId) {
        self.active.insert(id);
    }

    /// 动画结束或节点移除时注销。
    pub fn unregister(&mut self, id: NodeId) {
        self.active.remove(&id);
    }

    pub fn is_registered(&self, id: NodeId) -> bool {
        self.active.contains(&id)
    }

    /// 是否有进行中的动画/滚动惯性。
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// 返回当前活跃节点快照（避免迭代中修改集合）。
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
