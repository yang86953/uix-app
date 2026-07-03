//! RenderContext 扩展 — UI 域样式应用。

use uix_graphics::painting::PaintContext;
use uix_graphics::Radius;
use uix_platform::Rect;

use crate::style::Style;

/// 将 Style 应用到矩形区域（背景 + 边框 + 阴影 + 透明度）。
pub fn apply_style(ctx: &mut PaintContext<'_>, rect: Rect, style: &Style) {
    let r = if style.border_radius > 0.0 {
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
            r,
        );
    }
    if style.opacity < 1.0 {
        ctx.canvas_2d().set_opacity(style.opacity);
    }
    if let Some(bg) = style.background {
        ctx.fill_rect(rect, bg, r);
    }
    if style.border_width > 0.0 {
        if let Some(bc) = style.border_color {
            ctx.stroke_rect(rect, bc, style.border_width, r);
        }
    }
    if style.opacity < 1.0 {
        ctx.canvas_2d().set_opacity(1.0);
    }
}

/// 兼容 widget 对 `ctx.apply_style()` 的调用习惯。
pub trait RenderContextStyleExt {
    fn apply_style(&mut self, rect: Rect, style: &Style);
}

impl RenderContextStyleExt for PaintContext<'_> {
    fn apply_style(&mut self, rect: Rect, style: &Style) {
        apply_style(self, rect, style);
    }
}
