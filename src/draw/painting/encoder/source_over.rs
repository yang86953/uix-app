//! SrcOver 命令的安全分组证明与裁剪平移 — encoder 子模块。
//!
//! Picture splice 的代数安全性：透明中间合成等价性由「写区互不覆盖或
//! 被不透明 cover 完整覆盖」证明，几何裁剪/平移保持命令顺序与覆盖语义。

use std::sync::Arc;

use super::commands::{FrameCommand, FrameRasterOp};
use super::geometry::{
    FrameGlyphBlit, FrameImage, FrameRadius, FrameRect, FrameSampledRect, FrameStrokeRect,
};

struct SourceOverWrite {
    rect: FrameRect,
}

pub(super) fn stroke_visible_bounds(
    stroke: FrameStrokeRect,
    clip: FrameRect,
    width: i32,
    height: i32,
) -> Option<FrameRect> {
    if stroke.rect.is_empty() {
        return None;
    }
    let clip = clip.intersection(FrameRect::new(0, 0, width, height))?;
    let expand = stroke.line_width.value() * 0.5 + 1.0;
    let left = ((stroke.rect.x as f32 - expand).max(clip.x as f32)) as i32;
    let top = ((stroke.rect.y as f32 - expand).max(clip.y as f32)) as i32;
    let right = ((stroke.rect.x.saturating_add(stroke.rect.width) as f32 + expand)
        .min(clip.x.saturating_add(clip.width) as f32)) as i32;
    let bottom = ((stroke.rect.y.saturating_add(stroke.rect.height) as f32 + expand)
        .min(clip.y.saturating_add(clip.height) as f32)) as i32;
    (left < right && top < bottom).then(|| FrameRect::new(left, top, right - left, bottom - top))
}

pub(super) fn stroke_batch_bounds(
    strokes: &[FrameStrokeRect],
    clip: FrameRect,
    width: i32,
    height: i32,
) -> Option<(FrameRect, i64)> {
    let mut union = None;
    let mut covered_area = 0i64;
    for stroke in strokes {
        let Some(bounds) = stroke_visible_bounds(*stroke, clip, width, height) else {
            continue;
        };
        covered_area = covered_area
            .saturating_add(i64::from(bounds.width).saturating_mul(i64::from(bounds.height)));
        union = Some(union.map_or(bounds, |previous| {
            union_nonempty_frame_rect(previous, bounds)
        }));
    }
    union.map(|bounds| (bounds, covered_area))
}

pub(super) fn stroke_batches_can_merge(
    previous: &[FrameStrokeRect],
    next: &[FrameStrokeRect],
    clip: FrameRect,
    width: i32,
    height: i32,
) -> bool {
    const MAX_CLUSTER_ITEMS: usize = 2048;
    const MAX_CLUSTER_PIXELS: i64 = 1024 * 1024;
    const MAX_UNION_INFLATION: i64 = 4;
    if previous.len().saturating_add(next.len()) > MAX_CLUSTER_ITEMS {
        return false;
    }
    let Some((previous_bounds, previous_area)) = stroke_batch_bounds(previous, clip, width, height)
    else {
        return true;
    };
    let Some((next_bounds, next_area)) = stroke_batch_bounds(next, clip, width, height) else {
        return true;
    };
    for previous_stroke in previous {
        let Some(previous_visible) = stroke_visible_bounds(*previous_stroke, clip, width, height)
        else {
            continue;
        };
        for next_stroke in next {
            if stroke_visible_bounds(*next_stroke, clip, width, height)
                .and_then(|next_visible| previous_visible.intersection(next_visible))
                .is_some()
            {
                return false;
            }
        }
    }
    let union = union_nonempty_frame_rect(previous_bounds, next_bounds);
    let union_area = i64::from(union.width).saturating_mul(i64::from(union.height));
    let covered_area = previous_area.saturating_add(next_area);
    union_area <= MAX_CLUSTER_PIXELS
        && union_area <= covered_area.saturating_mul(MAX_UNION_INFLATION)
}

fn union_nonempty_frame_rect(a: FrameRect, b: FrameRect) -> FrameRect {
    let left = a.x.min(b.x);
    let top = a.y.min(b.y);
    let right = (i64::from(a.x) + i64::from(a.width)).max(i64::from(b.x) + i64::from(b.width));
    let bottom = (i64::from(a.y) + i64::from(a.height)).max(i64::from(b.y) + i64::from(b.height));
    FrameRect::new(
        left,
        top,
        (right - i64::from(left)) as i32,
        (bottom - i64::from(top)) as i32,
    )
}

pub(super) fn source_over_commands_have_safe_grouping(
    commands: &[FrameCommand],
    width: i32,
    height: i32,
) -> bool {
    let mut writes = Vec::new();
    let mut opaque_covers = Vec::new();
    for command in commands {
        match command {
            FrameCommand::Clear { .. } => return false,
            FrameCommand::Native { operation } => match operation {
                FrameRasterOp::FillRect { rect, color } => {
                    let opaque = color.a == u8::MAX;
                    if !push_source_over_write(&mut writes, *rect, width, height) {
                        return false;
                    }
                    if opaque {
                        push_opaque_cover(&mut opaque_covers, *rect);
                    }
                }
                FrameRasterOp::FillRoundedRect {
                    rect,
                    color,
                    radius,
                } => {
                    if !push_source_over_write(&mut writes, *rect, width, height) {
                        return false;
                    }
                    if color.a == u8::MAX {
                        if let Some(inner) = rounded_rect_opaque_inner(*rect, *radius) {
                            push_opaque_cover(&mut opaque_covers, inner);
                        }
                    }
                }
                FrameRasterOp::FillRoundedRectClipped {
                    rect,
                    color,
                    radius,
                    clip,
                } => {
                    let Some(visible) = rect.intersection(*clip) else {
                        continue;
                    };
                    if !push_source_over_write(&mut writes, visible, width, height) {
                        return false;
                    }
                    if color.a == u8::MAX {
                        if let Some(inner) = rounded_rect_opaque_inner(*rect, *radius)
                            .and_then(|inner| inner.intersection(*clip))
                        {
                            push_opaque_cover(&mut opaque_covers, inner);
                        }
                    }
                }
                FrameRasterOp::BlitGlyphs { glyphs, clip } => {
                    if !clip.is_within(width, height) {
                        return false;
                    }
                    for glyph in glyphs {
                        let (Ok(glyph_width), Ok(glyph_height)) =
                            (i32::try_from(glyph.width), i32::try_from(glyph.height))
                        else {
                            return false;
                        };
                        let Some(visible) =
                            FrameRect::new(glyph.x, glyph.y, glyph_width, glyph_height)
                                .intersection(*clip)
                        else {
                            continue;
                        };
                        if !push_source_over_write(&mut writes, visible, width, height) {
                            return false;
                        }
                    }
                }
                FrameRasterOp::StrokeRoundedRects { .. } => return false,
                FrameRasterOp::FillRectAdditive { rect, clip, .. } => {
                    // Additive 是目标相关 blend，但仍可作写区参与重叠证明：
                    // 与其它写区互不覆盖（或被不透明 cover 完整覆盖）时允许 splice。
                    let Some(visible) = rect.intersection(*clip) else {
                        // 完全被裁掉的操作没有写区。
                        continue;
                    };
                    // 只把真实可见交集加入重叠证明。
                    if !push_source_over_write(&mut writes, visible, width, height) {
                        return false;
                    }
                }
                FrameRasterOp::FillRoundedRectAdditive { rect, clip, .. } => {
                    // 圆角 shape 的保守写区也必须先受命令 clip 限制。
                    let Some(visible) = rect.intersection(*clip) else {
                        // 完全不可见时无需参与重叠证明。
                        continue;
                    };
                    // 可见包围盒仍保持安全保守性。
                    if !push_source_over_write(&mut writes, visible, width, height) {
                        return false;
                    }
                }
                FrameRasterOp::ScrollCopy { .. } => return false,
            },
            FrameCommand::CpuSegment { image, src, dst } => {
                if src.width != dst.width
                    || src.height != dst.height
                    || !src.is_within(image.width, image.height)
                    || !push_source_over_write(&mut writes, *dst, width, height)
                {
                    return false;
                }
                if image_src_is_fully_opaque(image, *src) {
                    push_opaque_cover(&mut opaque_covers, *dst);
                }
            }
            FrameCommand::PictureBlit {
                image,
                src,
                dst,
                opacity,
                additive,
            } => {
                let Some(integer_dst) = dst.as_integer() else {
                    // 亚像素 / 非整数缩放目标不能进入代数 splice。
                    return false;
                };
                // Additive Picture 依赖目标，不能参与 SrcOver splice 证明。
                if *additive
                    || src.width != integer_dst.width
                    || src.height != integer_dst.height
                    || !src.is_within(image.width, image.height)
                    || !push_source_over_write(&mut writes, integer_dst, width, height)
                {
                    return false;
                }
                if opacity.is_opaque() && image_src_is_fully_opaque(image, *src) {
                    push_opaque_cover(&mut opaque_covers, integer_dst);
                }
            }
        }
    }

    for (index, write) in writes.iter().enumerate() {
        for previous in &writes[..index] {
            if let Some(overlap) = previous.rect.intersection(write.rect) {
                if !opaque_covers_rect(&opaque_covers, overlap) {
                    return false;
                }
            }
        }
    }
    true
}

fn push_opaque_cover(covers: &mut Vec<FrameRect>, rect: FrameRect) {
    if covers.try_reserve(1).is_ok() {
        covers.push(rect);
    }
}

/// 判定 image 的 `src` 子矩形是否逐像素 alpha=255（premultiplied AARRGGBB）。
fn image_src_is_fully_opaque(image: &FrameImage, src: FrameRect) -> bool {
    if src.is_empty() || !src.is_within(image.width, image.height) {
        return false;
    }
    let pixels = image.pixels();
    let width = image.width as usize;
    let x0 = src.x as usize;
    let y0 = src.y as usize;
    let x1 = x0 + src.width as usize;
    let y1 = y0 + src.height as usize;
    for y in y0..y1 {
        let row = y * width;
        for x in x0..x1 {
            if pixels.get(row + x).copied().unwrap_or(0) >> 24 != 255 {
                return false;
            }
        }
    }
    true
}

fn opaque_covers_rect(covers: &[FrameRect], target: FrameRect) -> bool {
    if covers
        .iter()
        .any(|cover| cover.intersection(target) == Some(target))
    {
        return true;
    }

    let target_left = i64::from(target.x);
    let target_top = i64::from(target.y);
    let target_right = target_left + i64::from(target.width);
    let target_bottom = target_top + i64::from(target.height);
    let mut x = target_left;
    while x < target_right {
        let mut next_x = target_right;
        for cover in covers {
            let Some(clipped) = cover.intersection(target) else {
                continue;
            };
            let clipped_left = i64::from(clipped.x);
            let clipped_right = clipped_left + i64::from(clipped.width);
            for boundary in [clipped_left, clipped_right] {
                if boundary > x {
                    next_x = next_x.min(boundary);
                }
            }
        }

        let mut y = target_top;
        while y < target_bottom {
            let mut covered_until = y;
            for cover in covers {
                let cover_left = i64::from(cover.x);
                let cover_top = i64::from(cover.y);
                let cover_right = cover_left + i64::from(cover.width);
                let cover_bottom = cover_top + i64::from(cover.height);
                if cover_left <= x && cover_right >= next_x && cover_top <= y && cover_bottom > y {
                    covered_until = covered_until.max(cover_bottom.min(target_bottom));
                }
            }
            if covered_until == y {
                return false;
            }
            y = covered_until;
        }
        x = next_x;
    }
    true
}

fn rounded_rect_opaque_inner(rect: FrameRect, radius: FrameRadius) -> Option<FrameRect> {
    let radius = radius.to_radius();
    let inset = |value: f32| value.ceil() as i64;
    let left = inset(radius.tl.max(radius.bl));
    let right = inset(radius.tr.max(radius.br));
    let top = inset(radius.tl.max(radius.tr));
    let bottom = inset(radius.bl.max(radius.br));
    let inner_width = i64::from(rect.width)
        .checked_sub(left)?
        .checked_sub(right)?;
    let inner_height = i64::from(rect.height)
        .checked_sub(top)?
        .checked_sub(bottom)?;
    if inner_width <= 0 || inner_height <= 0 {
        return None;
    }
    Some(FrameRect::new(
        i32::try_from(i64::from(rect.x).checked_add(left)?).ok()?,
        i32::try_from(i64::from(rect.y).checked_add(top)?).ok()?,
        i32::try_from(inner_width).ok()?,
        i32::try_from(inner_height).ok()?,
    ))
}

fn push_source_over_write(
    writes: &mut Vec<SourceOverWrite>,
    rect: FrameRect,
    width: i32,
    height: i32,
) -> bool {
    if !rect.is_within(width, height) || writes.try_reserve(1).is_err() {
        return false;
    }
    writes.push(SourceOverWrite { rect });
    true
}

#[derive(Clone, Copy)]
pub(super) struct PictureCropTranslation {
    pub(super) source_width: i32,
    pub(super) source_height: i32,
    pub(super) source_crop: FrameRect,
    pub(super) dx: i32,
    pub(super) dy: i32,
    pub(super) target_width: i32,
    pub(super) target_height: i32,
}

pub(super) fn crop_and_translate_source_over_command(
    command: &FrameCommand,
    translation: &PictureCropTranslation,
) -> Result<Option<FrameCommand>, ()> {
    let translate_original = |rect: FrameRect| {
        if !rect.is_within(translation.source_width, translation.source_height) {
            return Err(());
        }
        rect.translated(translation.dx, translation.dy).ok_or(())
    };
    let translate_visible = |rect: FrameRect| {
        if !rect.is_within(translation.source_width, translation.source_height) {
            return Err(());
        }
        let Some(visible) = rect.intersection(translation.source_crop) else {
            return Ok(None);
        };
        let translated = visible
            .translated(translation.dx, translation.dy)
            .ok_or(())?;
        if !translated.is_within(translation.target_width, translation.target_height) {
            return Err(());
        }
        Ok(Some((visible, translated)))
    };

    Ok(Some(match command {
        FrameCommand::Clear { .. } => return Err(()),
        FrameCommand::Native { operation } => {
            let operation = match operation {
                FrameRasterOp::FillRect { rect, color } => {
                    let Some((_, rect)) = translate_visible(*rect)? else {
                        return Ok(None);
                    };
                    FrameRasterOp::FillRect {
                        rect,
                        color: *color,
                    }
                }
                FrameRasterOp::FillRoundedRect {
                    rect,
                    color,
                    radius,
                } => {
                    let Some((visible, translated_visible)) = translate_visible(*rect)? else {
                        return Ok(None);
                    };
                    let translated_rect = translate_original(*rect)?;
                    if visible == *rect {
                        FrameRasterOp::FillRoundedRect {
                            rect: translated_rect,
                            color: *color,
                            radius: *radius,
                        }
                    } else {
                        FrameRasterOp::FillRoundedRectClipped {
                            rect: translated_rect,
                            color: *color,
                            radius: *radius,
                            clip: translated_visible,
                        }
                    }
                }
                FrameRasterOp::FillRoundedRectClipped {
                    rect,
                    color,
                    radius,
                    clip,
                } => {
                    let Some((_, translated_clip)) = translate_visible(*clip)? else {
                        return Ok(None);
                    };
                    if rect
                        .intersection(*clip)
                        .and_then(|visible| visible.intersection(translation.source_crop))
                        .is_none()
                    {
                        return Ok(None);
                    }
                    FrameRasterOp::FillRoundedRectClipped {
                        rect: translate_original(*rect)?,
                        color: *color,
                        radius: *radius,
                        clip: translated_clip,
                    }
                }
                FrameRasterOp::BlitGlyphs { glyphs, clip } => {
                    let Some((visible_clip, translated_clip)) = translate_visible(*clip)? else {
                        return Ok(None);
                    };
                    let mut translated_glyphs = Vec::new();
                    translated_glyphs
                        .try_reserve_exact(glyphs.len())
                        .map_err(|_| ())?;
                    for glyph in glyphs {
                        let width = i32::try_from(glyph.width).map_err(|_| ())?;
                        let height = i32::try_from(glyph.height).map_err(|_| ())?;
                        if FrameRect::new(glyph.x, glyph.y, width, height)
                            .intersection(visible_clip)
                            .is_none()
                        {
                            continue;
                        }
                        translated_glyphs.push(FrameGlyphBlit {
                            x: glyph.x.checked_add(translation.dx).ok_or(())?,
                            y: glyph.y.checked_add(translation.dy).ok_or(())?,
                            width: glyph.width,
                            height: glyph.height,
                            color: glyph.color,
                            coverage: Arc::clone(&glyph.coverage),
                        });
                    }
                    if translated_glyphs.is_empty() {
                        return Ok(None);
                    }
                    FrameRasterOp::BlitGlyphs {
                        glyphs: translated_glyphs,
                        clip: translated_clip,
                    }
                }
                FrameRasterOp::StrokeRoundedRects { .. } => return Err(()),
                FrameRasterOp::FillRectAdditive { rect, color, clip } => {
                    // 硬矩形填充可直接收窄为 clip 与 Picture crop 的可见交集。
                    let Some(visible) = rect.intersection(*clip) else {
                        // 原命令完全不可见。
                        return Ok(None);
                    };
                    // 把可见交集平移到目标 Picture 坐标。
                    let Some((_, translated)) = translate_visible(visible)? else {
                        // Picture crop 进一步裁空时丢弃该命令。
                        return Ok(None);
                    };
                    FrameRasterOp::FillRectAdditive {
                        rect: translated,
                        color: *color,
                        // 收窄后的 rect 本身就是精确裁剪。
                        clip: translated,
                    }
                }
                FrameRasterOp::FillRoundedRectAdditive {
                    rect,
                    color,
                    radius,
                    clip,
                } => {
                    // 圆角几何不能收窄，否则会移动圆角中心；只平移其可见裁剪。
                    let Some(visible) = rect.intersection(*clip) else {
                        // 原命令完全不可见。
                        return Ok(None);
                    };
                    // Picture crop 负责进一步限制可见裁剪。
                    let Some((_, translated_clip)) = translate_visible(visible)? else {
                        // crop 后没有任何覆盖。
                        return Ok(None);
                    };
                    FrameRasterOp::FillRoundedRectAdditive {
                        // 保留原始圆角矩形并整体平移。
                        rect: translate_original(*rect)?,
                        color: *color,
                        radius: *radius,
                        // 仅裁剪最终覆盖，不改变 SDF 几何。
                        clip: translated_clip,
                    }
                }
                FrameRasterOp::ScrollCopy { .. } => return Err(()),
            };
            FrameCommand::Native { operation }
        }
        FrameCommand::CpuSegment { image, src, dst } => {
            if src.width != dst.width
                || src.height != dst.height
                || !src.is_within(image.width, image.height)
            {
                return Err(());
            }
            let Some((visible_dst, translated_dst)) = translate_visible(*dst)? else {
                return Ok(None);
            };
            let source_x = src
                .x
                .checked_add(visible_dst.x.checked_sub(dst.x).ok_or(())?)
                .ok_or(())?;
            let source_y = src
                .y
                .checked_add(visible_dst.y.checked_sub(dst.y).ok_or(())?)
                .ok_or(())?;
            let translated_src =
                FrameRect::new(source_x, source_y, visible_dst.width, visible_dst.height);
            if !translated_src.is_within(image.width, image.height) {
                return Err(());
            }
            FrameCommand::CpuSegment {
                image: image.clone(),
                src: translated_src,
                dst: translated_dst,
            }
        }
        FrameCommand::PictureBlit {
            image,
            src,
            dst,
            opacity,
            additive,
        } => {
            let Some(integer_dst) = dst.as_integer() else {
                return Err(());
            };
            if src.width != integer_dst.width
                || src.height != integer_dst.height
                || !src.is_within(image.width, image.height)
            {
                return Err(());
            }
            let Some((visible_dst, translated_dst)) = translate_visible(integer_dst)? else {
                return Ok(None);
            };
            let source_x = src
                .x
                .checked_add(visible_dst.x.checked_sub(integer_dst.x).ok_or(())?)
                .ok_or(())?;
            let source_y = src
                .y
                .checked_add(visible_dst.y.checked_sub(integer_dst.y).ok_or(())?)
                .ok_or(())?;
            let translated_src =
                FrameRect::new(source_x, source_y, visible_dst.width, visible_dst.height);
            if !translated_src.is_within(image.width, image.height) {
                return Err(());
            }
            FrameCommand::PictureBlit {
                image: image.clone(),
                src: translated_src,
                dst: FrameSampledRect::from_integer(translated_dst),
                opacity: *opacity,
                additive: *additive,
            }
        }
    }))
}

#[cfg(test)]
mod tests {
    // 引入被测裁剪、写区分析和命令类型。
    use super::*;
    // 引入测试使用的源颜色。
    use crate::draw::Color;

    // Additive 写区分析必须只使用 rect 与 clip 的真实可见交集。
    #[test]
    fn additive_write_grouping_uses_clipped_bounds() {
        // 构造几何包围盒重叠、但 Additive 可见裁剪互不相交的命令流。
        let clipped_commands = [
            // 第一条命令的几何延伸到右侧，但只允许写入左侧三列。
            FrameCommand::Native {
                // 保存带局部裁剪的硬 Additive 矩形。
                operation: FrameRasterOp::FillRectAdditive {
                    // 几何包围盒覆盖 x=1..9。
                    rect: FrameRect::new(1, 1, 8, 4),
                    // 使用不透明绿色作为 Additive 源。
                    color: Color::green(),
                    // 实际写区只覆盖 x=1..4。
                    clip: FrameRect::new(1, 1, 3, 4),
                },
            },
            // 第二条半透明命令从 x=6 开始，因此与真实 Additive 写区无交集。
            FrameCommand::Native {
                // 保存一个不能充当不透明 cover 的普通矩形。
                operation: FrameRasterOp::FillRect {
                    // 该矩形只与未裁剪的 Additive 几何包围盒重叠。
                    rect: FrameRect::new(6, 1, 2, 4),
                    // 半透明颜色确保重叠时不能借助 opaque cover 放行。
                    color: Color::from_rgba(255, 0, 0, 128),
                },
            },
        ];
        // 真实写区互不覆盖时 Picture splice 分组应安全。
        assert!(source_over_commands_have_safe_grouping(
            &clipped_commands,
            12,
            8
        ));
        // 构造相同几何，但将 Additive clip 扩展到第二条命令上的命令流。
        let overlapping_commands = [
            // 第一条命令现在允许写入整个几何包围盒。
            FrameCommand::Native {
                // 保存会与后一命令产生真实写区重叠的 Additive 矩形。
                operation: FrameRasterOp::FillRectAdditive {
                    // 保持与前一个场景相同的几何。
                    rect: FrameRect::new(1, 1, 8, 4),
                    // 保持相同的 Additive 源色。
                    color: Color::green(),
                    // 扩大裁剪以包含 x=6..8 的重叠区域。
                    clip: FrameRect::new(1, 1, 8, 4),
                },
            },
            // 复用同一个半透明普通矩形。
            FrameCommand::Native {
                // 保存不能掩盖目标相关顺序的 SrcOver 写入。
                operation: FrameRasterOp::FillRect {
                    // 该矩形现在位于 Additive 的真实写区内。
                    rect: FrameRect::new(6, 1, 2, 4),
                    // 半透明源不提供不透明覆盖证明。
                    color: Color::from_rgba(255, 0, 0, 128),
                },
            },
        ];
        // 真实写区重叠且没有不透明 cover 时必须拒绝 Picture splice 分组。
        assert!(!source_over_commands_have_safe_grouping(
            &overlapping_commands,
            12,
            8
        ));
    }

    // Picture crop 平移必须收窄硬矩形，并为圆角矩形保留原始 SDF 几何。
    #[test]
    fn additive_picture_translation_preserves_clip_semantics() {
        // 定义从 12×8 Picture 裁出右侧区域并平移到更大目标的映射。
        let translation = PictureCropTranslation {
            // Picture 原始逻辑宽度。
            source_width: 12,
            // Picture 原始逻辑高度。
            source_height: 8,
            // crop 从 x=5 开始，进一步收窄原命令裁剪。
            source_crop: FrameRect::new(5, 0, 5, 8),
            // 目标位置向右平移十个逻辑像素。
            dx: 10,
            // 目标位置向下平移三个逻辑像素。
            dy: 3,
            // 目标宽度足以容纳完整平移几何。
            target_width: 30,
            // 目标高度足以容纳完整平移几何。
            target_height: 20,
        };
        // 构造同时受命令 clip 与 Picture crop 限制的硬 Additive 矩形。
        let hard_command = FrameCommand::Native {
            // 保存原始几何、颜色和局部裁剪。
            operation: FrameRasterOp::FillRectAdditive {
                // 原始矩形覆盖 x=2..10、y=1..7。
                rect: FrameRect::new(2, 1, 8, 6),
                // 使用绿色源色检查载荷保持。
                color: Color::green(),
                // 命令裁剪先将可见区域限制到 x=4..8、y=2..6。
                clip: FrameRect::new(4, 2, 4, 4),
            },
        };
        // 执行硬矩形的裁剪平移。
        let translated_hard =
            match crop_and_translate_source_over_command(&hard_command, &translation) {
                // 保存成功产生的命令。
                Ok(Some(command)) => command,
                // 非空且界内的映射不得被裁空或拒绝。
                result => panic!("hard additive translation should succeed: {result:?}"),
            };
        // 提取平移后的硬 Additive 载荷。
        let FrameCommand::Native {
            operation: FrameRasterOp::FillRectAdditive { rect, color, clip },
        } = translated_hard
        else {
            // 变体改变会丢失原始目标相关 blend 事实。
            panic!("expected translated hard additive command");
        };
        // 硬矩形可以精确收窄为 clip 与 crop 的平移交集。
        assert_eq!(rect, FrameRect::new(15, 5, 3, 4));
        // 收窄后的 rect 本身也应成为精确裁剪。
        assert_eq!(clip, rect);
        // 平移不得改变调用方源色。
        assert_eq!(color, Color::green());
        // 构造相同几何和裁剪的圆角 Additive 命令。
        let rounded_command = FrameCommand::Native {
            // 保存共享 SDF 圆角载荷。
            operation: FrameRasterOp::FillRoundedRectAdditive {
                // 圆角中心依赖这一完整原始矩形。
                rect: FrameRect::new(2, 1, 8, 6),
                // 保持与硬矩形相同的源色。
                color: Color::green(),
                // 零圆角仍走圆角变体，并便于直接比较载荷。
                radius: FrameRadius::zero(),
                // 保持与硬矩形相同的命令裁剪。
                clip: FrameRect::new(4, 2, 4, 4),
            },
        };
        // 执行圆角矩形的裁剪平移。
        let translated_rounded =
            match crop_and_translate_source_over_command(&rounded_command, &translation) {
                // 保存成功产生的命令。
                Ok(Some(command)) => command,
                // 非空且界内的映射不得被裁空或拒绝。
                result => panic!("rounded additive translation should succeed: {result:?}"),
            };
        // 提取平移后的圆角 Additive 载荷。
        let FrameCommand::Native {
            operation:
                FrameRasterOp::FillRoundedRectAdditive {
                    rect,
                    color,
                    radius,
                    clip,
                },
        } = translated_rounded
        else {
            // 变体改变会绕开共享 SDF 几何。
            panic!("expected translated rounded additive command");
        };
        // 圆角矩形必须整体平移，不能像硬矩形一样收窄几何。
        assert_eq!(rect, FrameRect::new(12, 4, 8, 6));
        // 只有真实可见交集应成为平移后的局部裁剪。
        assert_eq!(clip, FrameRect::new(15, 5, 3, 4));
        // 平移不得改变调用方源色。
        assert_eq!(color, Color::green());
        // 平移不得改变共享 SDF 的圆角载荷。
        assert_eq!(radius, FrameRadius::zero());
    }
}
