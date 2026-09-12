//! Style painting helpers used by UI widgets.

use crate::core::Rect;
use crate::draw::Radius;
use crate::ui::theme::style::Style;
use crate::ui::widget_runtime::paint_context::PaintContext;
// 隔离边框线型到 draw 公共描边契约的几何映射。
#[path = "border.rs"]
// 编译 UI System 私有的边框绘制实现。
mod border;
// 隔离背景图层值到 draw 公共绘制契约的映射。
#[path = "background.rs"]
// 编译 UI System 私有的背景图层绘制实现。
mod background;

// 计算盒阴影 spread 应用后的基准矩形与圆角。
fn spread_shadow_geometry(
    // 接收组件原始边框盒。
    rect: Rect,
    // 接收与组件边框一致的可选圆角。
    radius: Option<Radius>,
    // 接收向外为正、向内为负的扩张距离。
    spread: f32,
) -> Option<(Rect, Option<Radius>)> {
    // Rust 直接构造的非有限 spread 按零处理，避免向绘制层传播坏几何。
    let spread = if spread.is_finite() { spread } else { 0.0 };
    // 计算扩张后的宽度。
    let width = rect.w + spread * 2.0;
    // 计算扩张后的高度。
    let height = rect.h + spread * 2.0;
    // 完全内缩后的空几何不提交阴影命令。
    if width <= 0.0 || height <= 0.0 {
        // 用 None 表示没有可绘制基准盒。
        return None;
    }
    // 同步移动左上角以保持四边等距扩张。
    let spread_rect = Rect::new(rect.x - spread, rect.y - spread, width, height);
    // 让圆角随轮廓扩张并把负半径钳制为零。
    let spread_radius = radius.map(|radius| Radius {
        // 调整左上圆角。
        tl: (radius.tl + spread).max(0.0),
        // 调整右上圆角。
        tr: (radius.tr + spread).max(0.0),
        // 调整右下圆角。
        br: (radius.br + spread).max(0.0),
        // 调整左下圆角。
        bl: (radius.bl + spread).max(0.0),
    });
    // 返回可直接交给现有阴影原语的几何。
    Some((spread_rect, spread_radius))
}

/// Applies a `Style` to a rectangular area.
pub fn apply_style(ctx: &mut PaintContext, rect: Rect, style: &Style) {
    // 单值与四角两种输入统一经有效值入口解析（S4）。
    let radius = style.effective_border_radius();

    // 首项是最上层，按反向 painter order 复用已有原语与脏区计算。
    for shadow in style.effective_box_shadows().iter().rev() {
        let Some((shadow_rect, shadow_radius)) = spread_shadow_geometry(rect, radius, shadow.spread) else {
            continue;
        };
        // 复用现有 CPU/GPU 阴影原语绘制扩张后的基准盒。
        ctx.draw_box_shadow(
            // 传入应用 spread 后的阴影矩形。
            shadow_rect,
            // 保留声明的模糊半径。
            shadow.blur,
            // 保留声明的水平偏移。
            shadow.offset_x,
            // 保留声明的垂直偏移。
            shadow.offset_y,
            // 保留声明的阴影颜色。
            shadow.color,
            // 使用随 spread 同步变化的圆角。
            shadow_radius,
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
    // 未声明尺寸策略时图片保持固有尺寸。
    let background_size = style.effective_background_size();
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
            // 传入尺寸化平铺策略。
            background_size,
            // 传入有效圆角，背景图与渐变同一边界裁剪。
            radius,
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
    // 视觉透明度由 widget 树声明 opacity 经 scene 节点合成通道统一应用，
    // 组件内部不得再按属性重复折叠，避免文本与背景衰减系数不一致。
    paint_surface(ctx.as_draw_mut());
}
