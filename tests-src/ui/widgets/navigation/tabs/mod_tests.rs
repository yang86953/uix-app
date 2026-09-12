//! `src/ui/widgets/navigation/tabs/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Tabs） ——

impl Tabs {
    // 测试目标保留标签滚动偏移观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tab_scroll_offset(&self) -> f32 {
        self.tab_scroll_offset.get()
    }

    // 测试目标保留标签区域观测入口，供导航布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tab_rect_for_test(&self, index: usize) -> Option<Rect> {
        self.local_tab_rect(index)
    }

    // 测试目标保留新增标签按钮区域观测入口，供导航交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn add_rect_for_test(&self) -> Rect {
        self.add_rect()
    }
}
