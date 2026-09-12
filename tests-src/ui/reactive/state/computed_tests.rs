//! `src/ui/reactive/state/computed.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl<T: Clone + Send + Sync + 'static> Computed<T>） ——

impl<T: Clone + Send + Sync + 'static> Computed<T> {
    // 暴露私有下游计数供生命周期回归验证。
    #[cfg(test)]
    // 不形成公开 API。
    pub(crate) fn effect_subscriber_count(&self) -> usize {
        // 委托通用弱订阅表统计。
        effect::effect_tests::subscriber_count(&self.inner.subscribers)
    }

    // 暴露私有绘制端点计数供节点生命周期回归验证。
    #[cfg(test)]
    // 不形成公开 Computed API。
    pub(crate) fn paint_site_count(&self) -> usize {
        // 读取派生源当前仍保留的绘制端点数量。
        self.inner
            .paint_sites
            .lock()
            .map(|sites| sites.len())
            .unwrap_or_default()
    }

    // 读取同锁缓存条目供私有并发回归验证。
    #[cfg(test)]
    // 不形成公开 API。
    pub(crate) fn cache_snapshot(&self) -> (T, u64) {
        // 构造后缓存必定存在，若 panic 则暴露违反的内部不变量。
        let entry = self.cache_entry().expect("Computed 缓存应在构造后存在");
        // 返回同一锁快照中的值和 revision。
        (entry.value, entry.generation)
    }
}
