//! `src/ui/event/text_selection/per_node.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl PerNodeTextSelection） ——

impl PerNodeTextSelection {
    // 测试目标保留渲染行起点观测入口，供排版布局测试按需调用。
    #[cfg(test)]
    pub(crate) fn rendered_line_origins_for_test(&self) -> Vec<Point> {
        let glyph_xs = self.glyph_xs.borrow();
        self.line_info
            .borrow()
            .iter()
            .map(|line| {
                let x = glyph_xs.get(line.glyph_start).copied().unwrap_or_default();
                Point::new(x, line.y)
            })
            .collect()
    }
}
