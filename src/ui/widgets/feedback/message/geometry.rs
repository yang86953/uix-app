//! 消息几何辅助。

use crate::core::{Point, Rect};

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
