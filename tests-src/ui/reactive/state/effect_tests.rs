//! `src/ui/reactive/state/effect.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 统计存活订阅，供私有生命周期回归使用。
#[cfg(test)]
// 不扩展公开响应式 API。
pub(crate) fn subscriber_count(registry: &DependencySubscriberRegistry) -> usize {
    // 快照函数同时完成死亡弱引用清理。
    collect_subscribers(registry).len()
}

// —— 自源文件移入的扩展 impl（impl DependencySubscriberSnapshot） ——

impl DependencySubscriberSnapshot {
    // 暴露窄计数供私有生命周期回归使用。
    #[cfg(test)]
    fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::One(_) => 1,
            Self::Many(subscribers) => subscribers.len(),
        }
    }
}
