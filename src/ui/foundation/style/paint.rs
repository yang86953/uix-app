//! Style painting helpers used by UI widgets.

use crate::core::Rect;
use crate::draw::api::PaintContext;
use crate::draw::Radius;
use crate::ui::style::Style;

/// Applies a `Style` to a rectangular area.
pub fn apply_style(ctx: &mut PaintContext<'_>, rect: Rect, style: &Style) {
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

    let paint_surface = |ctx: &mut PaintContext<'_>| {
        if let Some(background) = style.background {
            ctx.fill_rect(rect, background.resolve(ctx.tokens()), radius);
        }
        if style.has_border() {
            if let Some(border_color) = style.border_color {
                ctx.stroke_rect(
                    rect,
                    border_color.resolve(ctx.tokens()),
                    style.stroke_width(),
                    radius,
                );
            }
        }
    };
    if style.opacity < 1.0 {
        ctx.with_opacity(style.opacity, paint_surface);
    } else {
        paint_surface(ctx);
    }
}
