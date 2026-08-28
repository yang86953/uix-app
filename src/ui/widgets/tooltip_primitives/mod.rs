//! 输入与反馈组件共享的提示气泡几何和绘制原语。

use crate::core::{Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::draw::{Color, Radius};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::TooltipPlacement;
use crate::ui::widgets::overlay::{
    OverlayArrowVisual, OverlayBubbleGeometry, OverlayPlacement, draw_overlay_arrow,
    normalize_rect, resolve_overlay_bubble,
};

// 保存共享提示气泡的静态几何与排版；数值唯一由同目录 UIX 声明。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TooltipBubbleVisual {
    pub(crate) font_size: f32,
    pub(crate) horizontal_padding: f32,
    pub(crate) height: f32,
    pub(crate) arrow_size: f32,
    pub(crate) arrow_gap: f32,
    pub(crate) plain_gap: f32,
    pub(crate) radius: f32,
    pub(crate) edge_overlap: f32,
    pub(crate) center_ratio: f32,
    pub(crate) fallback_offset_bubbles: f32,
    pub(crate) fallback_span_bubbles: f32,
}

crate::uix_items!("src/ui/widgets/tooltip_primitives/tooltip_primitives.uix");

// 保存提示气泡经过翻转与表面约束后的最终几何。
// 复用共享气泡几何：最终矩形与溢出比较后的实际方向绑定。
pub(crate) type TooltipGeometry = OverlayBubbleGeometry<TooltipPlacement>;

// 使用当前逻辑表面解析提示气泡的最终位置与尺寸。
pub(crate) fn resolve_tooltip_geometry(
    // 接收提示文字以计算自然宽度。
    text: &str,
    // 接收箭头开关以计算目标间距。
    arrow: bool,
    // 接收作者指定方向。
    placement: TooltipPlacement,
    // 接收提示目标矩形。
    frame: Rect,
    // 接收当前逻辑表面矩形。
    surface: Rect,
    // 返回经过翻转与边界约束的几何。
) -> TooltipGeometry {
    resolve_tooltip_geometry_with_visual_and_size(
        arrow,
        placement,
        frame,
        surface,
        tooltip_bubble_size_with_visual(text, TOOLTIP_BUBBLE_VISUAL),
        TOOLTIP_BUBBLE_VISUAL,
    )
}

// 使用预计算自然尺寸解析提示气泡，避免同帧重复文字度量。
pub(crate) fn resolve_tooltip_geometry_with_visual_and_size(
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
    surface: Rect,
    natural: Size,
    visual: TooltipBubbleVisual,
) -> TooltipGeometry {
    // 归一化目标矩形以阻断非有限布局值。
    let frame = normalize_rect(frame);
    // 归一化表面矩形并把负尺寸收敛为零。
    let surface = normalize_rect(surface);
    // 复用共享气泡定位解析：箭头开启时间距含箭头自身尺寸。
    resolve_overlay_bubble(
        placement,
        frame,
        surface,
        natural.w,
        natural.h,
        tooltip_gap(arrow, visual),
        visual.center_ratio,
    )
}

// 合并目标节点与提示气泡的重绘区域。
#[cfg(feature = "feedback")]
pub(crate) fn tooltip_dirty_rect(
    text: &str,
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
    surface: Rect,
) -> Rect {
    tooltip_dirty_rect_with_visual_and_size(
        arrow,
        placement,
        frame,
        surface,
        tooltip_bubble_size_with_visual(text, TOOLTIP_BUBBLE_VISUAL),
        TOOLTIP_BUBBLE_VISUAL,
    )
}

// 使用预计算自然尺寸合并目标节点与提示气泡的重绘区域。
#[cfg(feature = "feedback")]
pub(crate) fn tooltip_dirty_rect_with_visual_and_size(
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
    surface: Rect,
    natural: Size,
    visual: TooltipBubbleVisual,
) -> Rect {
    // 归一化目标矩形以避免非有限值扩散到脏区。
    let frame = normalize_rect(frame);
    // 合并目标与当前表面内的最终气泡矩形。
    frame.union(&tooltip_bubble_rect_with_visual_and_size(
        arrow, placement, frame, surface, natural, visual,
    ))
}

// 计算提示气泡的最终布局矩形。
pub(crate) fn tooltip_bubble_rect(
    text: &str,
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
    surface: Rect,
) -> Rect {
    tooltip_bubble_rect_with_visual_and_size(
        arrow,
        placement,
        frame,
        surface,
        tooltip_bubble_size_with_visual(text, TOOLTIP_BUBBLE_VISUAL),
        TOOLTIP_BUBBLE_VISUAL,
    )
}

// 使用预计算自然尺寸计算提示气泡的最终布局矩形。
pub(crate) fn tooltip_bubble_rect_with_visual_and_size(
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
    surface: Rect,
    natural: Size,
    visual: TooltipBubbleVisual,
) -> Rect {
    // 复用统一解析器并只返回最终气泡矩形。
    resolve_tooltip_geometry_with_visual_and_size(arrow, placement, frame, surface, natural, visual)
        .bubble
}

// 绘制提示气泡、可选箭头和居中文字，并返回实际占用矩形。
pub(crate) fn paint_tooltip_bubble(
    ctx: &mut PaintContext,
    text: &str,
    frame: Rect,
    placement: TooltipPlacement,
    bg: Color,
    text_color: Color,
    arrow: bool,
) -> Rect {
    paint_tooltip_bubble_with_visual_and_size(
        ctx,
        text,
        frame,
        placement,
        bg,
        text_color,
        arrow,
        tooltip_bubble_size_with_visual(text, TOOLTIP_BUBBLE_VISUAL),
        TOOLTIP_BUBBLE_VISUAL,
    )
}

// 使用预计算自然尺寸绘制提示气泡，避免绘制阶段重复文字度量。
#[allow(clippy::too_many_arguments)]
pub(crate) fn paint_tooltip_bubble_with_visual_and_size(
    ctx: &mut PaintContext,
    text: &str,
    frame: Rect,
    placement: TooltipPlacement,
    bg: Color,
    text_color: Color,
    arrow: bool,
    natural: Size,
    visual: TooltipBubbleVisual,
) -> Rect {
    // 归一化目标矩形，使箭头锚点与解析器使用同一输入。
    let frame = normalize_rect(frame);
    // 从绘制上下文读取当前逻辑表面尺寸。
    let surface_size = ctx.logical_surface_size();
    // 将逻辑表面归一到窗口坐标原点。
    let surface = Rect::new(0.0, 0.0, surface_size.w, surface_size.h);
    // 使用与布局登记相同的共享解析器。
    let geometry = resolve_tooltip_geometry_with_visual_and_size(
        arrow, placement, frame, surface, natural, visual,
    );
    // 读取最终气泡矩形。
    let tip_frame = geometry.bubble;
    // 空表面不提交绘制命令。
    if tip_frame.w <= 0.0 || tip_frame.h <= 0.0 {
        // 返回空矩形供脏区缓存跳过。
        return Rect::zero();
    }
    // 将气泡、箭头与文字统一裁到当前表面。
    ctx.push_clip(surface);
    // 绘制最终气泡背景。
    ctx.fill_rect(tip_frame, bg, Some(Radius::uniform(visual.radius)));

    // 仅在启用箭头时绘制方向指示。
    if arrow {
        // 复用共享箭头绘制：嵌入量与尖端比率由 tooltip 视觉表声明。
        draw_overlay_arrow(
            ctx,
            frame,
            tip_frame,
            geometry.placement.decompose().0,
            bg,
            OverlayArrowVisual {
                // 使用共享箭头尺寸。
                size: visual.arrow_size,
                // 箭头底边嵌入气泡边缘，消除抗锯齿缝隙。
                edge_overlap: visual.edge_overlap,
                // 尖端按中心比例落在底边包围盒上。
                tip_ratio: visual.center_ratio,
                // 锚点跟随目标中心的比例。
                center_ratio: visual.center_ratio,
            },
        );
    }

    // 将超长文字裁在受限气泡矩形内。
    ctx.push_clip(tip_frame);
    // 在最终气泡矩形中居中文字。
    ctx.text_center(text, tip_frame, text_color, visual.font_size);
    // 恢复气泡文字裁剪。
    ctx.pop_clip();
    // 恢复逻辑表面裁剪。
    ctx.pop_clip();
    // 返回实际绘制的气泡矩形。
    tip_frame
}

// 根据文字度量计算提示气泡尺寸。
pub(crate) fn tooltip_bubble_size_with_visual(text: &str, visual: TooltipBubbleVisual) -> Size {
    let text_width = estimate_text_metrics(text, f32::INFINITY, visual.font_size).max_line_width;
    Size::new(text_width + visual.horizontal_padding, visual.height)
}

// 构造尚未获得窗口表面时的有限几何回退。
pub(crate) fn tooltip_fallback_surface(text: &str, frame: Rect) -> Rect {
    tooltip_fallback_surface_with_visual_and_size(
        frame,
        tooltip_bubble_size_with_visual(text, TOOLTIP_BUBBLE_VISUAL),
        TOOLTIP_BUBBLE_VISUAL,
    )
}

// 使用预计算自然尺寸构造尚未获得窗口表面时的有限几何回退。
pub(crate) fn tooltip_fallback_surface_with_visual_and_size(
    frame: Rect,
    bubble: Size,
    visual: TooltipBubbleVisual,
) -> Rect {
    // 归一化目标矩形以保持回退表面有限。
    let frame = normalize_rect(frame);
    // 在目标四周预留足以容纳两个方向候选的空间。
    frame.union(&Rect::new(
        // 从目标左侧两个气泡宽度开始。
        frame.x - bubble.w * visual.fallback_offset_bubbles,
        // 从目标上方两个气泡高度开始。
        frame.y - bubble.h * visual.fallback_offset_bubbles,
        // 横向覆盖目标与五个气泡宽度。
        bubble.w * visual.fallback_span_bubbles + frame.w,
        // 纵向覆盖目标与五个气泡高度。
        bubble.h * visual.fallback_span_bubbles + frame.h,
    ))
}

// 根据箭头可见性计算气泡与目标之间的间距。
fn tooltip_gap(arrow: bool, visual: TooltipBubbleVisual) -> f32 {
    if arrow {
        visual.arrow_size + visual.arrow_gap
    } else {
        visual.plain_gap
    }
}
