//! `src/ui/widgets/display/rich_text/rich_text_methods.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl RichText） ——

impl RichText {
    // 测试目标保留代码复制区域观测入口。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn code_copy_rect_for_test(&self, index: usize) -> Option<Rect> {
        self.code_regions
            .borrow()
            .get(index)
            .map(|region| region.rect)
    }

    // 测试目标保留布局行文本观测入口。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn layout_line_texts_for_test(&self) -> Vec<String> {
        self.layout_lines
            .borrow()
            .iter()
            .map(|line| line.glyphs.iter().map(|glyph| glyph.ch).collect())
            .collect()
    }

    // 测试目标保留链接命中点观测入口。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn link_point_for_test(&self, ordinal: usize) -> Option<Point> {
        // 解析目标链接的公开段索引。
        let segment_idx = self.link_segment_at_ordinal(ordinal)?;
        // 查找该段第一个真实字形的中心点。
        self.layout_lines.borrow().iter().find_map(|line| {
            line.glyphs
                .iter()
                .find(|glyph| glyph.segment_idx == segment_idx)
                .map(|glyph| Point::new(glyph.x + glyph.width * 0.5, line.y + line.height * 0.5))
        })
    }
}
