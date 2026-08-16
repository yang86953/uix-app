// 文件拖放逐窗注册表 Component 独占可接受 Drag Enter 的窗口集合。

// 哈希集合提供幂等启用与常数时间查询。
use std::collections::HashSet;
// 泛型窗口身份只需稳定哈希与相等语义。
use std::hash::Hash;

// 纯状态 Component 不持有任何 Wayland proxy 或 callback。
pub(super) struct FileDropWindowRegistry<T>
where
    // 窗口身份必须可作为集合键。
    T: Eq + Hash,
{
    // 集合是逐窗接收资格的唯一事实 owner。
    enabled: HashSet<T>,
}

// 空注册表不要求窗口身份本身实现 Default。
impl<T> Default for FileDropWindowRegistry<T>
where
    // HashSet 的键约束保持一致。
    T: Eq + Hash,
{
    // 构造无启用窗口的初始状态。
    fn default() -> Self {
        // 只初始化唯一集合字段。
        Self {
            // 新 backend 不默认接受外部文件。
            enabled: HashSet::new(),
        }
    }
}

impl<T> FileDropWindowRegistry<T>
where
    // 所有操作只依赖集合键语义。
    T: Eq + Hash,
{
    // 幂等发布一个窗口的接收资格。
    pub(super) fn insert(&mut self, window_id: T) {
        // 重复插入不会复制状态。
        self.enabled.insert(window_id);
    }

    // 幂等撤销一个窗口的接收资格。
    pub(super) fn remove(&mut self, window_id: &T) {
        // 不存在的窗口保持空操作。
        self.enabled.remove(window_id);
    }

    // 查询 Enter 是否可以继续进行 MIME 协商。
    pub(super) fn contains(&self, window_id: &T) -> bool {
        // 只读取唯一集合事实。
        self.enabled.contains(window_id)
    }

    // backend shutdown 一次撤销全部窗口资格。
    pub(super) fn clear(&mut self) {
        // 集合清空保持幂等。
        self.enabled.clear();
    }
}

// 纯状态测试不依赖 Wayland 连接或平台窗口。
#[cfg(test)]
mod tests {
    // 引入逐窗注册表 Component。
    use super::FileDropWindowRegistry;

    // 重复启用与禁用必须保持单一布尔事实。
    #[test]
    fn window_registration_is_idempotent() {
        // 使用整数代替平台 WindowId 验证纯集合语义。
        let mut registry = FileDropWindowRegistry::default();
        // 初始窗口没有接收资格。
        assert!(!registry.contains(&7_u64));
        // 首次启用发布资格。
        registry.insert(7_u64);
        // 重复启用不得改变可观察结果。
        registry.insert(7_u64);
        // 窗口现在可接受拖放。
        assert!(registry.contains(&7_u64));
        // 首次禁用撤销资格。
        registry.remove(&7_u64);
        // 重复禁用保持无资格。
        registry.remove(&7_u64);
        // 最终状态必须为禁用。
        assert!(!registry.contains(&7_u64));
    }

    // backend shutdown 必须同时撤销所有窗口资格。
    #[test]
    fn clear_revokes_every_window() {
        // 建立含两个窗口的注册表。
        let mut registry = FileDropWindowRegistry::default();
        // 启用第一窗口。
        registry.insert(1_u64);
        // 启用第二窗口。
        registry.insert(2_u64);
        // shutdown 执行全量撤销。
        registry.clear();
        // 第一窗口不再启用。
        assert!(!registry.contains(&1_u64));
        // 第二窗口也不再启用。
        assert!(!registry.contains(&2_u64));
    }
}
