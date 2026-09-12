//! `src/app/queues/main_thread_queue.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl MainThreadQueue） ——

impl MainThreadQueue {
    // 测试目标保留主线程队列长度观测入口，供队列行为测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.pending.lock().unwrap_or_else(|e| e.into_inner()).len()
    }
}
