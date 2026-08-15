//! 集中式动画系统驱动节点的活动登记表。

use std::collections::HashSet;

use crate::draw::scene::NodeId;

/// 无需遍历完整组件树即可追踪活动动画节点。
#[derive(Debug, Clone, Default)]
pub struct AnimationRegistry {
    active: HashSet<NodeId>,
}

impl AnimationRegistry {
    /// 创建不含活动节点的登记表。
    pub fn new() -> Self {
        Self::default()
    }

    /// 幂等登记一个具有集中式动画工作的节点。
    pub fn register(&mut self, id: NodeId) {
        self.active.insert(id);
    }

    /// 注销节点；节点不存在时保持不变。
    pub fn unregister(&mut self, id: NodeId) {
        self.active.remove(&id);
    }

    /// 判断指定节点是否具有活动动画工作。
    pub fn is_registered(&self, id: NodeId) -> bool {
        self.active.contains(&id)
    }

    /// 判断登记表中是否存在任意活动节点。
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// 返回当前活动节点身份的无序快照。
    pub fn active_ids(&self) -> Vec<NodeId> {
        self.active.iter().copied().collect()
    }

    /// 注销全部活动节点。
    pub fn clear(&mut self) {
        self.active.clear();
    }
}
