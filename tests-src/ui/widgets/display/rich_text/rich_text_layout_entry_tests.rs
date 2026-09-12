//! `src/ui/widgets/display/rich_text/rich_text_layout_entry.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

/// 基于真实字体度量执行富文本布局。
#[cfg(test)]
pub(crate) fn layout_rich_text_real(
    // 接收公开段列表。
    segments: &[RichTextSegment],
    // 接收最大行宽。
    max_width: f32,
    // 接收默认字号。
    default_font_size: f32,
    // 接收默认颜色。
    default_color: Color,
    // 接收字体服务。
    font_service: &FontService,
    // 接收字体句柄。
    font: &FontHandle,
) -> (Vec<LayoutLine>, f32, f32) {
    // 测试兼容入口同样使用稳定占位图片状态。
    super::super::rich_text_layout::layout_rich_text_real_with_images(
        // 传递公开段列表。
        segments,
        // 传递可用宽度。
        max_width,
        // 传递默认字号。
        default_font_size,
        // 从测试或兼容调用方默认色派生无主题调色板。
        RichTextPalette::estimated(default_color),
        // 传递字体服务。
        font_service,
        // 传递字体句柄。
        font,
        // 使用空图片状态表。
        &InlineImageStates::new(),
    )
}