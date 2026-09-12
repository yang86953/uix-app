//! `src/ui/widgets/display/rich_text/rich_text_layout.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 返回围绕文本顶部的局部斜体仿射变换。
#[cfg(test)]
pub(crate) fn italic_transform(pivot_y: f32) -> Transform {
    italic_transform_with_shear(pivot_y, RICH_TEXT_VISUAL.metrics.italic_shear)
}
