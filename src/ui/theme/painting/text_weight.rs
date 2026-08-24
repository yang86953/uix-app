// 引入字体后端产出的中性布局与共享所有权。
use std::sync::Arc;

use crate::draw::resources::font::text_backend::TextLayout;
// 引入绘制坐标、颜色与 UI 绘制上下文。
use crate::{core::Point, draw::Color, ui::widget_runtime::paint_context::PaintContext};
// 引入 UI System 自有的字体粗细契约。
use crate::ui::theme::style::FontWeight;

// 按最终字重选择当前字体后端可提供的最接近字体面并提交字形。
pub(crate) fn paint(
    // 接收 UI 绘制上下文，但不向 draw System 泄漏 FontWeight。
    ctx: &mut PaintContext<'_, '_>,
    // 借用测量、选择与装饰共用的字形布局。
    layout: &Arc<TextLayout>,
    // 接收绝对绘制原点。
    position: Point,
    // 接收最终文字颜色。
    color: Color,
    // 接收最终字体尺寸。
    font_size: f32,
    // 接收已经解析的 UI 字重。
    font_weight: FontWeight,
) {
    // 常规面与粗体面都先提交基础字形。
    ctx.blit_shared_glyph_layout(Arc::clone(layout), position, color, font_size);
    // 当前后端只有单一字体面时，六百及以上使用稳定水平偏移合成粗体。
    if font_weight.uses_bold_face() {
        // 第二次提交保持布局几何不变，只增加笔画视觉厚度。
        ctx.blit_shared_glyph_layout(
            // 复用同一字形布局。
            Arc::clone(layout),
            // 使用与既有 strong 契约一致的亚像素偏移。
            Point::new(position.x + 0.6, position.y),
            // 保持同一文字颜色。
            color,
            // 保持同一字号。
            font_size,
        );
    }
}
