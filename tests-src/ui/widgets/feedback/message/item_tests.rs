//! `src/ui/widgets/feedback/message/item.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl MessageHandle） ——

impl MessageHandle {
    // 由 owner 或聚焦测试按程序化原因关闭声明条目。
    #[cfg(test)]
    pub(crate) fn close_declaration(&self, id: u64) -> bool {
        // 外部声明 ID 的关闭观察器会建立 tombstone。
        self.queue.remove_external(id)
    }
}
