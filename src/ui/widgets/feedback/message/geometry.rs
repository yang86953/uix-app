//! 消息几何辅助。

use crate::core::{Point, Rect};
use crate::draw::Color;

pub(crate) struct MessageGeometry {
    pub(crate) icon: Rect,
    pub(crate) content: Rect,
    pub(crate) action: Option<Rect>,
    pub(crate) close: Rect,
}

pub(super) fn transitioned_rect(rect: Rect, offset: Point, scale: f32) -> Rect {
    let scale = scale.max(0.0);
    let width = rect.w * scale;
    let height = rect.h * scale;
    Rect::new(
        rect.x + (rect.w - width) * 0.5 + offset.x,
        rect.y + (rect.h - height) * 0.5 + offset.y,
        width,
        height,
    )
}

pub(super) fn translated_rect(rect: Rect, offset: Point) -> Rect {
    Rect::new(rect.x + offset.x, rect.y + offset.y, rect.w, rect.h)
}

pub(super) fn expand_rect(rect: Rect, margin: f32) -> Rect {
    if rect.w <= 0.0 || rect.h <= 0.0 {
        Rect::zero()
    } else {
        Rect::new(
            rect.x - margin,
            rect.y - margin,
            rect.w + margin * 2.0,
            rect.h + margin * 2.0,
        )
    }
}

pub(super) fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}
