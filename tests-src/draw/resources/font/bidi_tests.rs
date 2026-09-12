//! `src/draw/resources/font/bidi.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl BidiAnalysis） ——

impl BidiAnalysis {
    /// 返回全部段落及其自动解析的基准级别。
    #[cfg(test)]
    pub(crate) fn paragraphs(&self) -> &[BidiParagraph] {
        // 只读借用稳定段落表。
        &self.paragraphs
    }

    /// 按逻辑字符索引输入对象，返回应用 L1/L2 后的视觉顺序与级别。
    #[cfg(test)]
    pub(crate) fn line_order(
        // 借用共享段落分析。
        &self,
        // 当前视觉行覆盖的逻辑字符范围。
        line_range: Range<usize>,
        // 每个待排对象对应的逻辑字符起点，必须按逻辑顺序输入。
        logical_char_indices: &[usize],
    ) -> BidiLineOrder {
        // 兼容单次调用方，同时让多行调用方可以显式复用上下文。
        self.line_order_context()
            .line(line_range)
            .order(logical_char_indices)
    }
}
