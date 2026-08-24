//! 输入与反馈组件共享的提示气泡几何和绘制原语。

use crate::core::{Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::TooltipPlacement;

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
#[derive(Debug, Clone, Copy, PartialEq)]
// 将气泡矩形与实际使用方向绑定，供绘制箭头复用。
pub(crate) struct TooltipGeometry {
    // 记录最终可见气泡矩形。
    pub(crate) bubble: Rect,
    // 记录溢出比较后实际采用的方向。
    pub(crate) placement: TooltipPlacement,
}

// 计算提示气泡在目标节点四个方向上的左上角。
fn tooltip_origin(
    frame: Rect,
    placement: TooltipPlacement,
    text_w: f32,
    text_h: f32,
    gap: f32,
    center_ratio: f32,
) -> (f32, f32) {
    match placement {
        TooltipPlacement::Top => (
            frame.x + frame.w * center_ratio - text_w * center_ratio,
            frame.y - text_h - gap,
        ),
        TooltipPlacement::Bottom => (
            frame.x + frame.w * center_ratio - text_w * center_ratio,
            frame.y + frame.h + gap,
        ),
        TooltipPlacement::Left => (
            frame.x - text_w - gap,
            frame.y + frame.h * center_ratio - text_h * center_ratio,
        ),
        TooltipPlacement::Right => (
            frame.x + frame.w + gap,
            frame.y + frame.h * center_ratio - text_h * center_ratio,
        ),
    }
}

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
    let frame = normalize_tooltip_rect(frame);
    // 归一化表面矩形并把负尺寸收敛为零。
    let surface = normalize_tooltip_rect(surface);
    // 将气泡宽度限制在当前表面内。
    let width = natural.w.min(surface.w).max(0.0);
    // 将气泡高度限制在当前表面内。
    let height = natural.h.min(surface.h).max(0.0);
    // 空表面不生成可见气泡。
    if width <= 0.0 || height <= 0.0 {
        // 返回保留作者方向的空几何。
        return TooltipGeometry {
            // 空矩形不会参与绘制或命中。
            bubble: Rect::zero(),
            // 保留方向便于调用方稳定处理。
            placement,
        };
    }

    // 计算与作者方向相反的候选方向。
    let flipped = flip_tooltip_placement(placement);
    // 计算作者方向的未约束候选矩形。
    let authored = tooltip_rect_for_placement(frame, placement, arrow, width, height, visual);
    // 计算反向候选的未约束矩形。
    let alternate = tooltip_rect_for_placement(frame, flipped, arrow, width, height, visual);
    // 选择总越界量更小的方向，平局时保持作者配置。
    let (candidate, resolved) =
        // 仅当反向候选严格更优时翻转。
        if tooltip_overflow_score(alternate, surface) < tooltip_overflow_score(authored, surface) {
            // 使用反向候选及其方向。
            (alternate, flipped)
        } else {
            // 保留作者候选及其方向。
            (authored, placement)
        };
    // 计算气泡横向可用的最大起点。
    let max_x = surface.x + surface.w - width;
    // 计算气泡纵向可用的最大起点。
    let max_y = surface.y + surface.h - height;

    // 返回约束到表面内部的最终几何。
    TooltipGeometry {
        // 同时约束两个轴，处理交叉轴溢出与超长文字。
        bubble: Rect::new(
            // 约束横坐标到表面范围。
            candidate.x.clamp(surface.x, max_x),
            // 约束纵坐标到表面范围。
            candidate.y.clamp(surface.y, max_y),
            // 使用已受限宽度。
            width,
            // 使用已受限高度。
            height,
        ),
        // 暴露实际方向供箭头朝向复用。
        placement: resolved,
    }
}

// 计算指定方向下尚未约束的气泡矩形。
fn tooltip_rect_for_placement(
    // 接收目标矩形。
    frame: Rect,
    // 接收候选方向。
    placement: TooltipPlacement,
    // 接收箭头开关。
    arrow: bool,
    // 接收已受限宽度。
    width: f32,
    // 接收已受限高度。
    height: f32,
    // 接收调用方声明的共享视觉表。
    visual: TooltipBubbleVisual,
    // 返回候选矩形。
) -> Rect {
    // 根据箭头状态计算目标间距。
    let gap = tooltip_gap(arrow, visual);
    // 根据方向计算候选左上角。
    let (x, y) = tooltip_origin(frame, placement, width, height, gap, visual.center_ratio);
    // 组装候选矩形。
    Rect::new(x, y, width, height)
}

// 计算候选矩形越出逻辑表面的总距离。
fn tooltip_overflow_score(rect: Rect, surface: Rect) -> f32 {
    // 累加左、上、右、下四个方向的正越界量。
    (surface.x - rect.x).max(0.0)
        // 累加上边界越界量。
        + (surface.y - rect.y).max(0.0)
        // 累加右边界越界量。
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        // 累加下边界越界量。
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

// 返回提示方向的主轴反向候选。
fn flip_tooltip_placement(placement: TooltipPlacement) -> TooltipPlacement {
    // 按上下或左右成对翻转。
    match placement {
        // 顶部空间不足时尝试底部。
        TooltipPlacement::Top => TooltipPlacement::Bottom,
        // 底部空间不足时尝试顶部。
        TooltipPlacement::Bottom => TooltipPlacement::Top,
        // 左侧空间不足时尝试右侧。
        TooltipPlacement::Left => TooltipPlacement::Right,
        // 右侧空间不足时尝试左侧。
        TooltipPlacement::Right => TooltipPlacement::Left,
    }
}

// 归一化提示目标或逻辑表面矩形。
fn normalize_tooltip_rect(rect: Rect) -> Rect {
    // 替换非有限坐标并收敛负尺寸。
    Rect::new(
        // 非有限横坐标回退到原点。
        if rect.x.is_finite() { rect.x } else { 0.0 },
        // 非有限纵坐标回退到原点。
        if rect.y.is_finite() { rect.y } else { 0.0 },
        // 非有限或负宽度收敛为零。
        if rect.w.is_finite() {
            // 保留有限非负宽度。
            rect.w.max(0.0)
        } else {
            // 非有限宽度回退为零。
            0.0
        },
        // 非有限或负高度收敛为零。
        if rect.h.is_finite() {
            // 保留有限非负高度。
            rect.h.max(0.0)
        } else {
            // 非有限高度回退为零。
            0.0
        },
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
    let frame = normalize_tooltip_rect(frame);
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
    let frame = normalize_tooltip_rect(frame);
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
        // 使用共享箭头尺寸。
        let arrow_sz = visual.arrow_size;
        // 按最终方向计算箭头包围盒。
        let (ax, ay, aw, ah) = match geometry.placement {
            // 顶部气泡的箭头从下边缘指向目标。
            TooltipPlacement::Top => (
                tooltip_arrow_anchor(
                    frame.x + frame.w * visual.center_ratio,
                    tip_frame.x,
                    tip_frame.w,
                    arrow_sz,
                    visual.center_ratio,
                ) - arrow_sz,
                tip_frame.y + tip_frame.h - visual.edge_overlap,
                arrow_sz * 2.0,
                arrow_sz,
            ),
            // 底部气泡的箭头从上边缘指向目标。
            TooltipPlacement::Bottom => (
                tooltip_arrow_anchor(
                    frame.x + frame.w * visual.center_ratio,
                    tip_frame.x,
                    tip_frame.w,
                    arrow_sz,
                    visual.center_ratio,
                ) - arrow_sz,
                tip_frame.y - arrow_sz + visual.edge_overlap,
                arrow_sz * 2.0,
                arrow_sz,
            ),
            // 左侧气泡的箭头从右边缘指向目标。
            TooltipPlacement::Left => (
                tip_frame.x + tip_frame.w - visual.edge_overlap,
                tooltip_arrow_anchor(
                    frame.y + frame.h * visual.center_ratio,
                    tip_frame.y,
                    tip_frame.h,
                    arrow_sz,
                    visual.center_ratio,
                ) - arrow_sz,
                arrow_sz,
                arrow_sz * 2.0,
            ),
            // 右侧气泡的箭头从左边缘指向目标。
            TooltipPlacement::Right => (
                tip_frame.x - arrow_sz + visual.edge_overlap,
                tooltip_arrow_anchor(
                    frame.y + frame.h * visual.center_ratio,
                    tip_frame.y,
                    tip_frame.h,
                    arrow_sz,
                    visual.center_ratio,
                ) - arrow_sz,
                arrow_sz,
                arrow_sz * 2.0,
            ),
        };
        // 使用最终方向绘制箭头。
        draw_arrow(
            ctx,
            ax,
            ay,
            aw,
            ah,
            geometry.placement,
            bg,
            visual.center_ratio,
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
    let frame = normalize_tooltip_rect(frame);
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

// 将箭头锚点限制在气泡边缘的安全范围内。
fn tooltip_arrow_anchor(
    desired: f32,
    start: f32,
    length: f32,
    inset: f32,
    center_ratio: f32,
) -> f32 {
    // 极窄气泡无法保留两侧 inset 时使用边缘中心。
    if length <= inset * 2.0 {
        // 返回当前边缘中心。
        start + length * center_ratio
    } else {
        // 将目标中心限制在安全边缘范围。
        desired.clamp(start + inset, start + length - inset)
    }
}

// 绘制指向目标节点的三角箭头。
fn draw_arrow(
    ctx: &mut PaintContext,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    dir: TooltipPlacement,
    color: Color,
    center_ratio: f32,
) {
    let (x1, y1, x2, y2, x3, y3) = match dir {
        TooltipPlacement::Top => (x, y, x + w, y, x + w * center_ratio, y + h),
        TooltipPlacement::Bottom => (x, y + h, x + w, y + h, x + w * center_ratio, y),
        TooltipPlacement::Left => (x, y, x, y + h, x + w, y + h * center_ratio),
        TooltipPlacement::Right => (x + w, y, x + w, y + h, x, y + h * center_ratio),
    };
    let mut path = PathBuilder::new();
    path.move_to(x1, y1);
    path.line_to(x2, y2);
    path.line_to(x3, y3);
    path.close();
    ctx.fill_path(&path.build(), color, FillRule::NonZero);
}

// 仅在测试构建中编译共享提示气泡几何契约。
#[cfg(test)]
// 将契约放在原语模块内以直接覆盖私有尺寸计算。
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/ui/widgets/tooltip_primitives__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
