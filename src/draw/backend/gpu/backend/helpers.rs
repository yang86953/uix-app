//! 帧编码几何辅助 — backend 子模块。

use crate::draw::painting::{FrameGlyphBlit, FrameRect};

pub(super) fn glyph_visible_bounds(
    glyph: &FrameGlyphBlit,
    clip: FrameRect,
    target_width: i32,
    target_height: i32,
) -> Option<FrameRect> {
    let width = i32::try_from(glyph.width()).ok()?;
    let height = i32::try_from(glyph.height()).ok()?;
    FrameRect::new(glyph.x(), glyph.y(), width, height)
        .intersection(clip)
        .and_then(|bounds| bounds.intersection(FrameRect::new(0, 0, target_width, target_height)))
}

pub(super) fn union_frame_rect_wide(a: FrameRect, b: FrameRect) -> FrameRect {
    let left = i64::from(a.x).min(i64::from(b.x));
    let top = i64::from(a.y).min(i64::from(b.y));
    let right = (i64::from(a.x) + i64::from(a.width)).max(i64::from(b.x) + i64::from(b.width));
    let bottom = (i64::from(a.y) + i64::from(a.height)).max(i64::from(b.y) + i64::from(b.height));
    FrameRect::new(
        left as i32,
        top as i32,
        (right - left) as i32,
        (bottom - top) as i32,
    )
}
