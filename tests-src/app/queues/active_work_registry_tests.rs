//! `src/app/queues/active_work_registry.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl ActiveWorkRegistry） ——

impl ActiveWorkRegistry {
    #[cfg(test)]
    pub(crate) fn drain_due(&mut self, now: Instant) -> Vec<ActiveWorkKind> {
        let mut due = Vec::new();
        // 测试便捷入口保持“取出全部到期工作”的既有语义。
        self.drain_due_into_with_budget(now, &mut due, usize::MAX);
        due
    }
}
