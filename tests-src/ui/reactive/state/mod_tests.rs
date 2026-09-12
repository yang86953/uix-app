//! `src/ui/reactive/state/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl<T: Clone + Send + Sync + 'static> State<T>） ——

impl<T: Clone + Send + Sync + 'static> State<T> {
    // 暴露测试专用的活跃 Effect 订阅数量以验证租约生命周期。
    #[cfg(test)]
    // 此计数仅用于模块私有测试，不构成公开 State 契约。
    pub(crate) fn effect_subscriber_count(&self) -> usize {
        // 委托独立注册表清理死亡弱引用并读取精确数量。
        effect::effect_tests::subscriber_count(&self.source.subscribers)
    }

    // 暴露测试专用的活跃绘制站点数量以验证节点租约释放。
    #[cfg(test)]
    // 此计数仅用于模块私有回归，不构成公开 State 契约。
    pub(crate) fn paint_site_count(&self) -> usize {
        // 读取当前共享状态槽仍保留的绘制端点数量。
        self.paint_sites
            .lock()
            .map(|sites| sites.len())
            .unwrap_or_default()
    }
}
