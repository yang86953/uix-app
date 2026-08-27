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
