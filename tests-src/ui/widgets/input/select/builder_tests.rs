//! `src/ui/widgets/input/select/builder.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Select） ——

impl Select {
    // 测试目标保留多选移除区域观测入口，供选择器交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn first_multi_remove_rect(&self) -> Option<Rect> {
        self.multi_remove_rects
            .borrow()
            .first()
            .map(|(_, rect)| *rect)
    }
}
