//! `src/ui/widget_runtime/app_state.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl AppState） ——

impl AppState {
    // 测试目标保留语义事件队列长度观测入口，供语义事件测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn pending_semantic_event_count(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .semantic_events
            .len()
    }
}
