//! 气泡类浮层相对触发器的共享 placement 机制。
//!
//! 单一实现承载方向候选原点、主轴翻转、溢出评分、表面钳制与箭头绘制；
//! tooltip、popover、popconfirm 的公开 placement 枚举通过 [`OverlayPlacement`]
//! 接入。各枚举的 `flip` 反向双射与 `decompose` 方向映射是该枚举的固有声明，
//! 保留逐分支手写以维持公开枚举与既有行为零改动。

use crate::core::Rect;
use crate::draw::{Color, FillRule, PathBuilder};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::widgets::overlay_types::TooltipPlacement;

// 浮层气泡相对触发器的主轴方向。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlaySide {
    // 触发器上方。
    Top,
    // 触发器下方。
    Bottom,
    // 触发器左侧。
    Left,
    // 触发器右侧。
    Right,
}

// 浮层气泡在正交轴上相对触发器的对齐。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OverlayAlign {
    // 组件族默认对齐：垂直方向跟随触发器左缘，水平方向按中心比例居中。
    Lead,
    // 与触发器起点边缘对齐。
    Start,
    // 按中心比例与触发器居中对齐。
    Center,
    // 与触发器终点边缘对齐。
    End,
}

// 组件自有 placement 枚举接入共享机制的最小映射。
pub(crate) trait OverlayPlacement: Copy + PartialEq {
    // 拆解为共享方向与正交对齐。
    fn decompose(self) -> (OverlaySide, OverlayAlign);
    // 返回主轴反向候选，正交对齐保持不变。
    fn flip(self) -> Self;
}

// Tooltip 的四向变体全部按中心比例正交居中。
impl OverlayPlacement for TooltipPlacement {
    fn decompose(self) -> (OverlaySide, OverlayAlign) {
        match self {
            TooltipPlacement::Top => (OverlaySide::Top, OverlayAlign::Center),
            TooltipPlacement::Bottom => (OverlaySide::Bottom, OverlayAlign::Center),
            TooltipPlacement::Left => (OverlaySide::Left, OverlayAlign::Center),
            TooltipPlacement::Right => (OverlaySide::Right, OverlayAlign::Center),
        }
    }

    fn flip(self) -> Self {
        match self {
            TooltipPlacement::Top => TooltipPlacement::Bottom,
            TooltipPlacement::Bottom => TooltipPlacement::Top,
            TooltipPlacement::Left => TooltipPlacement::Right,
            TooltipPlacement::Right => TooltipPlacement::Left,
        }
    }
}

// Popover 的族默认变体在垂直方向跟随触发器左缘、在水平方向居中。
#[cfg(feature = "feedback")]
impl OverlayPlacement for crate::ui::widgets::feedback::popover::PopoverPlacement {
    fn decompose(self) -> (OverlaySide, OverlayAlign) {
        use crate::ui::widgets::feedback::popover::PopoverPlacement;
        match self {
            PopoverPlacement::Top => (OverlaySide::Top, OverlayAlign::Lead),
            PopoverPlacement::TopLeft => (OverlaySide::Top, OverlayAlign::Start),
            PopoverPlacement::TopRight => (OverlaySide::Top, OverlayAlign::End),
            PopoverPlacement::Bottom => (OverlaySide::Bottom, OverlayAlign::Lead),
            PopoverPlacement::BottomLeft => (OverlaySide::Bottom, OverlayAlign::Start),
            PopoverPlacement::BottomRight => (OverlaySide::Bottom, OverlayAlign::End),
            PopoverPlacement::Left => (OverlaySide::Left, OverlayAlign::Center),
            PopoverPlacement::LeftTop => (OverlaySide::Left, OverlayAlign::Start),
            PopoverPlacement::LeftBottom => (OverlaySide::Left, OverlayAlign::End),
            PopoverPlacement::Right => (OverlaySide::Right, OverlayAlign::Center),
            PopoverPlacement::RightTop => (OverlaySide::Right, OverlayAlign::Start),
            PopoverPlacement::RightBottom => (OverlaySide::Right, OverlayAlign::End),
        }
    }

    fn flip(self) -> Self {
        use crate::ui::widgets::feedback::popover::PopoverPlacement;
        match self {
            PopoverPlacement::Top => PopoverPlacement::Bottom,
            PopoverPlacement::TopLeft => PopoverPlacement::BottomLeft,
            PopoverPlacement::TopRight => PopoverPlacement::BottomRight,
            PopoverPlacement::Bottom => PopoverPlacement::Top,
            PopoverPlacement::BottomLeft => PopoverPlacement::TopLeft,
            PopoverPlacement::BottomRight => PopoverPlacement::TopRight,
            PopoverPlacement::Left => PopoverPlacement::Right,
            PopoverPlacement::LeftTop => PopoverPlacement::RightTop,
            PopoverPlacement::LeftBottom => PopoverPlacement::RightBottom,
            PopoverPlacement::Right => PopoverPlacement::Left,
            PopoverPlacement::RightTop => PopoverPlacement::LeftTop,
            PopoverPlacement::RightBottom => PopoverPlacement::LeftBottom,
        }
    }
}

// Popconfirm 只声明垂直方向的六个变体，族默认对齐同样跟随触发器左缘。
#[cfg(feature = "feedback")]
impl OverlayPlacement for crate::ui::widgets::feedback::popconfirm::PopconfirmPlacement {
    fn decompose(self) -> (OverlaySide, OverlayAlign) {
        use crate::ui::widgets::feedback::popconfirm::PopconfirmPlacement;
        match self {
            PopconfirmPlacement::Top => (OverlaySide::Top, OverlayAlign::Lead),
            PopconfirmPlacement::TopLeft => (OverlaySide::Top, OverlayAlign::Start),
            PopconfirmPlacement::TopRight => (OverlaySide::Top, OverlayAlign::End),
            PopconfirmPlacement::Bottom => (OverlaySide::Bottom, OverlayAlign::Lead),
            PopconfirmPlacement::BottomLeft => (OverlaySide::Bottom, OverlayAlign::Start),
            PopconfirmPlacement::BottomRight => (OverlaySide::Bottom, OverlayAlign::End),
        }
    }

    fn flip(self) -> Self {
        use crate::ui::widgets::feedback::popconfirm::PopconfirmPlacement;
        match self {
            PopconfirmPlacement::Top => PopconfirmPlacement::Bottom,
            PopconfirmPlacement::TopLeft => PopconfirmPlacement::BottomLeft,
            PopconfirmPlacement::TopRight => PopconfirmPlacement::BottomRight,
            PopconfirmPlacement::Bottom => PopconfirmPlacement::Top,
            PopconfirmPlacement::BottomLeft => PopconfirmPlacement::TopLeft,
            PopconfirmPlacement::BottomRight => PopconfirmPlacement::TopRight,
        }
    }
}

// 计算方向候选在主轴与正交轴上的未约束左上角。
pub(crate) fn overlay_candidate_origin<P: OverlayPlacement>(
    placement: P,
    frame: Rect,
    width: f32,
    height: f32,
    gap: f32,
    center_ratio: f32,
) -> (f32, f32) {
    let (side, align) = placement.decompose();
    // 垂直方向的正交轴是横轴，水平方向的正交轴是纵轴。
    let vertical = matches!(side, OverlaySide::Top | OverlaySide::Bottom);
    let (cross_pos, cross_len) = if vertical {
        (frame.x, frame.w)
    } else {
        (frame.y, frame.h)
    };
    let popup_cross = if vertical { width } else { height };
    // 正交轴对齐：族默认在垂直方向保持触发器左缘，其余情形按边缘或中心比对。
    let cross = match align {
        OverlayAlign::Start => cross_pos,
        OverlayAlign::End => cross_pos + cross_len - popup_cross,
        // 垂直方向的族默认对齐跟随触发器左缘（上下弹层的既有惯例）。
        OverlayAlign::Lead if vertical => cross_pos,
        // 水平方向的族默认与显式居中都按中心比例对齐。
        OverlayAlign::Lead | OverlayAlign::Center => {
            cross_pos + cross_len * center_ratio - popup_cross * center_ratio
        }
    };
    match side {
        OverlaySide::Top => (cross, frame.y - height - gap),
        OverlaySide::Bottom => (cross, frame.y + frame.h + gap),
        OverlaySide::Left => (frame.x - width - gap, cross),
        OverlaySide::Right => (frame.x + frame.w + gap, cross),
    }
}

// 计算候选矩形越出逻辑表面的总距离。
pub(crate) fn overlay_overflow_score(rect: Rect, surface: Rect) -> f32 {
    // 累加左、上、右、下四个方向的正越界量。
    (surface.x - rect.x).max(0.0)
        + (surface.y - rect.y).max(0.0)
        + (rect.x + rect.w - surface.x - surface.w).max(0.0)
        + (rect.y + rect.h - surface.y - surface.h).max(0.0)
}

// 保存气泡经过翻转与表面约束后的最终几何；气泡矩形与实际方向绑定。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OverlayBubbleGeometry<P: OverlayPlacement> {
    // 记录最终可见气泡矩形。
    pub(crate) bubble: Rect,
    // 记录溢出比较后实际采用的方向。
    pub(crate) placement: P,
}

// 共享气泡定位解析：尺寸收敛、方向翻转、溢出评分与表面钳制的单一实现。
// 调用方先完成矩形归一化与间距计算；仅当反向候选严格更优时翻转，平局保持作者方向。
pub(crate) fn resolve_overlay_bubble<P: OverlayPlacement>(
    placement: P,
    frame: Rect,
    surface: Rect,
    preferred_width: f32,
    preferred_height: f32,
    gap: f32,
    center_ratio: f32,
) -> OverlayBubbleGeometry<P> {
    // 将气泡尺寸限制在当前表面内。
    let width = preferred_width.min(surface.w).max(0.0);
    let height = preferred_height.min(surface.h).max(0.0);
    // 空表面不生成可见气泡。
    if width <= 0.0 || height <= 0.0 {
        // 返回保留作者方向的空几何。
        return OverlayBubbleGeometry {
            // 空矩形不会参与绘制或命中。
            bubble: Rect::zero(),
            // 保留方向便于调用方稳定处理。
            placement,
        };
    }
    // 计算与作者方向相反的候选方向。
    let flipped = placement.flip();
    // 计算作者方向的未约束候选矩形。
    let (authored_x, authored_y) =
        overlay_candidate_origin(placement, frame, width, height, gap, center_ratio);
    let authored = Rect::new(authored_x, authored_y, width, height);
    // 计算反向候选的未约束矩形。
    let (alternate_x, alternate_y) =
        overlay_candidate_origin(flipped, frame, width, height, gap, center_ratio);
    let alternate = Rect::new(alternate_x, alternate_y, width, height);
    // 选择总越界量更小的方向，平局时保持作者配置。
    let (candidate, resolved) =
        // 仅当反向候选严格更优时翻转。
        if overlay_overflow_score(alternate, surface) < overlay_overflow_score(authored, surface) {
            // 使用反向候选及其方向。
            (alternate, flipped)
        } else {
            // 保留作者候选及其方向。
            (authored, placement)
        };
    // 计算气泡起点在各轴可用的最大值。
    let max_x = surface.x + surface.w - width;
    let max_y = surface.y + surface.h - height;
    // 返回约束到表面内部的最终几何。
    OverlayBubbleGeometry {
        // 同时约束两个轴，处理交叉轴溢出与超长内容。
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

// 共享箭头绘制的视觉参数。
#[derive(Debug, Clone, Copy)]
pub(crate) struct OverlayArrowVisual {
    // 箭头底边半宽（三角腰长）。
    pub(crate) size: f32,
    // 箭头底边嵌入气泡边缘的深度，消除抗锯齿缝隙；无嵌入传 0。
    pub(crate) edge_overlap: f32,
    // 尖端在底边包围盒上的相对位置；居中传 0.5。
    pub(crate) tip_ratio: f32,
    // 锚点跟随触发器中心的比例。
    pub(crate) center_ratio: f32,
}

// 将箭头锚点限制在气泡边缘的安全范围内。
pub(crate) fn overlay_arrow_anchor(
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
        // 将触发器中心限制在安全边缘范围。
        desired.clamp(start + inset, start + length - inset)
    }
}

// 绘制指向触发节点的三角箭头的单一实现。
pub(crate) fn draw_overlay_arrow(
    ctx: &mut PaintContext,
    trigger: Rect,
    bubble: Rect,
    side: OverlaySide,
    color: Color,
    arrow: OverlayArrowVisual,
) {
    let size = arrow.size;
    let (x1, y1, x2, y2, x3, y3) = match side {
        OverlaySide::Top => {
            let anchor = overlay_arrow_anchor(
                trigger.x + trigger.w * arrow.center_ratio,
                bubble.x,
                bubble.w,
                size,
                arrow.center_ratio,
            );
            // 底边贴住气泡下缘并按需嵌入，尖端指向触发器。
            let base = bubble.y + bubble.h - arrow.edge_overlap;
            (
                anchor - size,
                base,
                anchor + size,
                base,
                anchor - size + size * 2.0 * arrow.tip_ratio,
                base + size,
            )
        }
        OverlaySide::Bottom => {
            let anchor = overlay_arrow_anchor(
                trigger.x + trigger.w * arrow.center_ratio,
                bubble.x,
                bubble.w,
                size,
                arrow.center_ratio,
            );
            let base = bubble.y + arrow.edge_overlap;
            (
                anchor - size,
                base,
                anchor + size,
                base,
                anchor - size + size * 2.0 * arrow.tip_ratio,
                base - size,
            )
        }
        OverlaySide::Left => {
            let anchor = overlay_arrow_anchor(
                trigger.y + trigger.h * arrow.center_ratio,
                bubble.y,
                bubble.h,
                size,
                arrow.center_ratio,
            );
            let base = bubble.x + bubble.w - arrow.edge_overlap;
            (
                base,
                anchor - size,
                base,
                anchor + size,
                base + size,
                anchor - size + size * 2.0 * arrow.tip_ratio,
            )
        }
        OverlaySide::Right => {
            let anchor = overlay_arrow_anchor(
                trigger.y + trigger.h * arrow.center_ratio,
                bubble.y,
                bubble.h,
                size,
                arrow.center_ratio,
            );
            let base = bubble.x + arrow.edge_overlap;
            (
                base,
                anchor - size,
                base,
                anchor + size,
                base - size,
                anchor - size + size * 2.0 * arrow.tip_ratio,
            )
        }
    };
    let mut path = PathBuilder::new();
    path.move_to(x1, y1);
    path.line_to(x2, y2);
    path.line_to(x3, y3);
    path.close();
    ctx.fill_path(&path.build(), color, FillRule::NonZero);
}
