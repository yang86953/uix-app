//! `src/ui/widgets/general/typography/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Typography） ——

impl Typography {
    // 测试目标保留复制按钮区域观测入口，供排版交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn copy_rect_for_test(&self) -> Option<Rect> {
        self.copy_rect.get()
    }

    // 测试目标保留渲染行起点观测入口，供排版布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn rendered_line_origins_for_test(&self) -> Vec<Point> {
        self.sel.rendered_line_origins_for_test()
    }
}
