//! Style painting helpers used by UI widgets.

use crate::core::Rect;
use crate::draw::Radius;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::style::Style;

/// Applies a `Style` to a rectangular area.
pub fn apply_style(ctx: &mut PaintContext, rect: Rect, style: &Style) {
    let radius = if style.border_radius > 0.0 {
        Some(Radius::uniform(style.border_radius))
    } else {
        None
    };

    if let Some(shadow) = style.box_shadow.as_ref() {
        ctx.draw_box_shadow(
            rect,
            shadow.blur,
            shadow.offset_x,
            shadow.offset_y,
            shadow.color,
            radius,
        );
    }

    // 令牌在 UI 边界解析为最终颜色；draw 层闭包只接收已解析绘制值。
    let background = style.background.map(|bg| bg.resolve(ctx.tokens()));
    let border_color = style.border_color.map(|bc| bc.resolve(ctx.tokens()));
    let paint_surface = |ctx: &mut crate::draw::api::PaintContext| {
        if let Some(background) = background {
            ctx.fill_rect(rect, background, radius);
        }
        if style.has_border() {
            if let Some(border_color) = border_color {
                ctx.stroke_rect(rect, border_color, style.stroke_width(), radius);
            }
        }
    };
    if style.opacity < 1.0 {
        ctx.with_opacity(style.opacity, paint_surface);
    } else {
        paint_surface(ctx.as_draw_mut());
    }
}
