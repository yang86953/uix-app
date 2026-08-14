//! RichText 无资源状态布局调用的兼容入口。

// 引入共享布局返回类型。
use super::LayoutLine;
// 引入图片状态表与公开段模型。
use super::{RichTextPalette, RichTextSegment, inline_image::InlineImageStates};
// 引入字体服务与句柄。
#[cfg(test)]
use crate::draw::resources::font::font_service::FontService;
// 引入布局颜色。
use crate::draw::Color;
// 测试兼容入口才需要字体句柄。
#[cfg(test)]
use crate::draw::FontHandle;

/// 执行富文本估算布局，返回 (行列表, 总高度, 最大行宽)。
pub(crate) fn layout_rich_text(
    // 接收公开段列表。
    segments: &[RichTextSegment],
    // 接收最大行宽。
    max_width: f32,
    // 接收默认字号。
    default_font_size: f32,
    // 接收默认颜色。
    default_color: Color,
) -> (Vec<LayoutLine>, f32, f32) {
    // 无组件资源状态的调用使用稳定占位几何。
    super::rich_text_layout::layout_rich_text_with_images(
        // 传递公开段列表。
        segments,
        // 传递可用宽度。
        max_width,
        // 传递默认字号。
        default_font_size,
        // 从测试或兼容调用方默认色派生无主题调色板。
        RichTextPalette::estimated(default_color),
        // 使用空图片状态表。
        &InlineImageStates::new(),
    )
}

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
    super::rich_text_layout::layout_rich_text_real_with_images(
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
