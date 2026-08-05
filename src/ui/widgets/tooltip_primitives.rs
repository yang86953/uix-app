//! 输入与反馈组件共享的提示气泡几何和绘制原语。

use crate::core::{Rect, Size};
use crate::draw::resources::font::text_backend::estimate_text_metrics;
use crate::draw::{Color, FillRule, PathBuilder, Radius};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::widgets::TooltipPlacement;

// 提示文字使用组件族既有的固定字号。
const TOOLTIP_FONT_SIZE: f32 = 12.0;
// 提示气泡为文字预留固定水平内边距。
const TOOLTIP_HORIZONTAL_PADDING: f32 = 16.0;
// 提示气泡保持既有固定高度。
const TOOLTIP_HEIGHT: f32 = 26.0;
// 提示箭头保持既有固定尺寸。
const TOOLTIP_ARROW_SIZE: f32 = 6.0;

// 计算提示气泡在目标节点四个方向上的左上角。
fn tooltip_origin(
    frame: Rect,
    placement: TooltipPlacement,
    text_w: f32,
    text_h: f32,
    gap: f32,
) -> (f32, f32) {
    match placement {
        TooltipPlacement::Top => (
            frame.x + frame.w * 0.5 - text_w * 0.5,
            frame.y - text_h - gap,
        ),
        TooltipPlacement::Bottom => (
            frame.x + frame.w * 0.5 - text_w * 0.5,
            frame.y + frame.h + gap,
        ),
        TooltipPlacement::Left => (
            frame.x - text_w - gap,
            frame.y + frame.h * 0.5 - text_h * 0.5,
        ),
        TooltipPlacement::Right => (
            frame.x + frame.w + gap,
            frame.y + frame.h * 0.5 - text_h * 0.5,
        ),
    }
}

// 合并目标节点与提示气泡的重绘区域。
#[cfg(feature = "feedback")]
pub(crate) fn tooltip_dirty_rect(
    text: &str,
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
) -> Rect {
    frame.union(&tooltip_bubble_rect(text, arrow, placement, frame))
}

// 计算提示气泡的最终布局矩形。
pub(crate) fn tooltip_bubble_rect(
    text: &str,
    arrow: bool,
    placement: TooltipPlacement,
    frame: Rect,
) -> Rect {
    let bubble = tooltip_bubble_size(text);
    let text_w = bubble.w;
    let text_h = bubble.h;
    let gap = tooltip_gap(arrow);
    let (tx, ty) = tooltip_origin(frame, placement, text_w, text_h, gap);
    Rect::new(tx, ty, text_w, text_h)
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
    let tip_frame = tooltip_bubble_rect(text, arrow, placement, frame);
    ctx.fill_rect(tip_frame, bg, Some(Radius::uniform(4.0)));

    if arrow {
        let arrow_sz = TOOLTIP_ARROW_SIZE;
        let (ax, ay, aw, ah) = match placement {
            TooltipPlacement::Top => (
                tip_frame.x + tip_frame.w * 0.5 - arrow_sz,
                tip_frame.y + tip_frame.h - 1.0,
                arrow_sz * 2.0,
                arrow_sz,
            ),
            TooltipPlacement::Bottom => (
                tip_frame.x + tip_frame.w * 0.5 - arrow_sz,
                tip_frame.y - arrow_sz + 1.0,
                arrow_sz * 2.0,
                arrow_sz,
            ),
            TooltipPlacement::Left => (
                tip_frame.x + tip_frame.w - 1.0,
                tip_frame.y + tip_frame.h * 0.5 - arrow_sz,
                arrow_sz,
                arrow_sz * 2.0,
            ),
            TooltipPlacement::Right => (
                tip_frame.x - arrow_sz + 1.0,
                tip_frame.y + tip_frame.h * 0.5 - arrow_sz,
                arrow_sz,
                arrow_sz * 2.0,
            ),
        };
        draw_arrow(ctx, ax, ay, aw, ah, placement, bg);
    }

    ctx.text_center(text, tip_frame, text_color, TOOLTIP_FONT_SIZE);
    tip_frame
}

// 根据文字度量计算提示气泡尺寸。
fn tooltip_bubble_size(text: &str) -> Size {
    let text_width = estimate_text_metrics(text, f32::INFINITY, TOOLTIP_FONT_SIZE).max_line_width;
    Size::new(text_width + TOOLTIP_HORIZONTAL_PADDING, TOOLTIP_HEIGHT)
}

// 根据箭头可见性计算气泡与目标之间的间距。
fn tooltip_gap(arrow: bool) -> f32 {
    if arrow {
        TOOLTIP_ARROW_SIZE + 2.0
    } else {
        4.0
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
) {
    let (x1, y1, x2, y2, x3, y3) = match dir {
        TooltipPlacement::Top => (x, y, x + w, y, x + w / 2.0, y + h),
        TooltipPlacement::Bottom => (x, y + h, x + w, y + h, x + w / 2.0, y),
        TooltipPlacement::Left => (x, y, x, y + h, x + w, y + h / 2.0),
        TooltipPlacement::Right => (x + w, y, x + w, y + h, x, y + h / 2.0),
    };
    let mut path = PathBuilder::new();
    path.move_to(x1, y1);
    path.line_to(x2, y2);
    path.line_to(x3, y3);
    path.close();
    ctx.fill_path(&path.build(), color, FillRule::NonZero);
}
