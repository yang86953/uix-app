//! `src/app/queues/app_timer.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl AppTimerQueue） ——

impl AppTimerQueue {
    // 测试目标保留完整 deadline 快照入口，供时钟行为测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn deadlines(&self) -> Vec<(TimerId, Instant)> {
        let mut deadlines = Vec::new();
        let Some(_) = self.deadlines_into_if_changed(None, &mut deadlines) else {
            // 已知修订为空时必返回新修订；返回 None 说明内部修订状态异常，
            // 附带当前已收集条目数便于定位。
            panic!(
                "initial timer deadline snapshot must report a revision (collected={})",
                deadlines.len()
            );
        };
        deadlines
    }

    // 测试目标保留队列长度观测入口，供定时器行为测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.inner
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .entries
            .len()
    }
}
