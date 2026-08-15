//! Style painting helpers used by UI widgets.

use crate::core::Rect;
use crate::draw::Radius;
use crate::ui::component::paint_context::PaintContext;
use crate::ui::theme::style::Style;
// 隔离边框线型到 draw 公共描边契约的几何映射。
#[path = "style_paint/border.rs"]
// 编译 UI System 私有的边框绘制实现。
mod border;
// 隔离背景图层值到 draw 公共绘制契约的映射。
#[path = "style_paint/background.rs"]
// 编译 UI System 私有的背景图层绘制实现。
mod background;

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
    // 背景图颜色也只在 UI System 边界解析主题令牌。
    let background_image =
        background::resolve_background(style.background_image.as_ref(), ctx.tokens());
    // 未声明定位时使用左上角有效默认值。
    let background_position = style.effective_background_position();
    // 未声明重复时使用双轴平铺有效默认值。
    let background_repeat = style.effective_background_repeat();
    let border_color = style.border_color.map(|bc| bc.resolve(ctx.tokens()));
    let paint_surface = |ctx: &mut crate::draw::painting::PaintContext| {
        if let Some(background) = background {
            ctx.fill_rect(rect, background, radius);
        }
        // 背景图层位于背景色之后且位于边框之前。
        background::paint_background(
            // 传入 draw System 的公开绘制上下文。
            ctx,
            // 传入当前组件背景盒。
            rect,
            // 传入已经解析颜色的单层背景图。
            background_image,
            // 传入确定的二维定位。
            background_position,
            // 传入确定的重复方式。
            background_repeat,
        );
        if style.has_border() {
            if let Some(border_color) = border_color {
                // UI System 在私有边界内解释 CSS 线型并调用 draw 公共契约。
                border::paint_border(
                    // 传入 draw System 的公开绘制上下文。
                    ctx,
                    // 传入当前组件布局矩形。
                    rect,
                    // 传入已经解析的最终边框颜色。
                    border_color,
                    // 保持现有四边最大宽度描边契约。
                    style.stroke_width(),
                    // 传入圆角以保持线型与现有实线几何一致。
                    radius,
                    // 传入确定的 UI 有效线型。
                    style.effective_border_style(),
                );
            }
        }
    };
    if style.opacity < 1.0 {
        ctx.with_opacity(style.opacity, paint_surface);
    } else {
        paint_surface(ctx.as_draw_mut());
    }
}
