//! CPU 参考执行与像素级光栅操作 — encoder 子模块。
//!
//! 参考执行器（[`super::FrameEncoder::render_reference`]）与 CPU backend /
//! GPU readback 回退共用的像素填充、blit 与 blend 原语。

use crate::core::Rect;
use crate::draw::geometry::types::BlendMode;
use crate::draw::raster::software_rasterizer::SoftwareRasterizer;
use crate::draw::Color;

use super::commands::FrameRasterOp;
use super::error::FrameEncoderError;
use super::geometry::{FrameImage, FrameRadius, FrameRect, FrameSampledRect, FrameStrokeRect};

pub(super) fn full_frame_image_blit(
    width: i32,
    height: i32,
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
) -> bool {
    image.width == width
        && image.height == height
        && src == FrameRect::new(0, 0, width, height)
        && dst == FrameRect::new(0, 0, width, height)
}

pub(crate) fn pixel_len(width: i32, height: i32) -> Result<usize, FrameEncoderError> {
    if width <= 0 || height <= 0 {
        return Err(FrameEncoderError::InvalidExtent { width, height });
    }
    let pixels = i64::from(width) * i64::from(height);
    usize::try_from(pixels).map_err(|_| FrameEncoderError::InvalidExtent { width, height })
}

pub(super) fn apply_stroke_rect_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    stroke: FrameStrokeRect,
    clip: FrameRect,
    additive: bool,
) {
    if stroke.rect.is_empty() {
        return;
    }
    // Additive 必须直接读写已有目标像素，复用 SoftwareRasterizer 的真实 blend 规则。
    if additive {
        // 为当前目标构造无变换的参考 rasterizer。
        let mut renderer = SoftwareRasterizer::new(width, height);
        // 选择逐通道饱和加法混合。
        renderer.set_blend_mode(BlendMode::Additive);
        // FrameRasterOp 的 clip 已经位于 surface 空间。
        renderer.push_clip_surface(Rect::new(
            clip.x as f32,
            clip.y as f32,
            clip.width as f32,
            clip.height as f32,
        ));
        // 直接在累计目标上执行与 Canvas2D 一致的描边。
        renderer.stroke_rect(
            pixels,
            width,
            height,
            Rect::new(
                stroke.rect.x as f32,
                stroke.rect.y as f32,
                stroke.rect.width as f32,
                stroke.rect.height as f32,
            ),
            stroke.color,
            stroke.line_width.value(),
            Some(stroke.radius.to_radius()),
        );
        // Additive 已经完成，禁止继续执行 SrcOver 低层路径。
        return;
    }
    // 普通描边继续复用既有低层 SrcOver 光栅函数。
    crate::draw::raster::rasterizer::stroke::stroke_rect(
        pixels,
        width,
        height,
        Rect::new(
            clip.x as f32,
            clip.y as f32,
            clip.width as f32,
            clip.height as f32,
        ),
        1.0,
        Rect::new(
            stroke.rect.x as f32,
            stroke.rect.y as f32,
            stroke.rect.width as f32,
            stroke.rect.height as f32,
        ),
        stroke.color,
        stroke.line_width.value(),
        Some(stroke.radius.to_radius()),
    );
}

pub(super) fn apply_raster_op_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    operation: &FrameRasterOp,
) {
    match operation {
        FrameRasterOp::FillRect { rect, color } => {
            fill_rect_pixels(width, height, pixels, *rect, *color)
        }
        FrameRasterOp::FillRoundedRect {
            rect,
            color,
            radius,
        } => fill_rounded_rect_pixels(width, height, pixels, *rect, *color, *radius),
        FrameRasterOp::FillRoundedRectClipped {
            rect,
            color,
            radius,
            clip,
        } => {
            if let Some(clip) = clip.intersection(FrameRect::new(0, 0, width, height)) {
                fill_rounded_rect_pixels_clipped(
                    width, height, pixels, *rect, *color, *radius, clip,
                )
            }
        }
        FrameRasterOp::BlitGlyphs { glyphs, clip } => {
            let clip = Rect::new(
                clip.x as f32,
                clip.y as f32,
                clip.width as f32,
                clip.height as f32,
            );
            for glyph in glyphs {
                crate::draw::raster::rasterizer::glyph::blit_glyph(
                    pixels,
                    width,
                    height,
                    clip,
                    1.0,
                    glyph.x,
                    glyph.y,
                    glyph.coverage.as_ref(),
                    glyph.width as usize,
                    glyph.height as usize,
                    glyph.color,
                );
            }
        }
        FrameRasterOp::StrokeRoundedRects {
            strokes,
            clip,
            additive,
        } => {
            for stroke in strokes {
                // 对累计目标应用命令携带的统一 blend 事实。
                apply_stroke_rect_pixels(width, height, pixels, *stroke, *clip, *additive);
            }
        }
        FrameRasterOp::FillRectAdditive { rect, color, clip } => {
            // 对累计目标执行硬矩形裁剪后的饱和加法。
            fill_rect_additive_pixels(width, height, pixels, *rect, *color, *clip)
        }
        FrameRasterOp::FillRoundedRectAdditive {
            rect,
            color,
            radius,
            clip,
        } => {
            // 先把命令裁剪限制到参考 surface，完全不可见时保持 no-op。
            if let Some(clip) = clip.intersection(FrameRect::new(0, 0, width, height)) {
                // 保留原始圆角几何，仅裁剪最终覆盖。
                fill_rounded_rect_additive_pixels(
                    width, height, pixels, *rect, *color, *radius, clip,
                )
            }
        }
        FrameRasterOp::ScrollCopy { viewport, dx, dy } => {
            scroll_copy_pixels(width, height, pixels, *viewport, *dx, *dy)
        }
    }
}

/// Applies one destination-dependent (or ordinary) raster op onto an existing
/// premultiplied pixel buffer. Used by CPU execution and by NativeGpu when it
/// lowers Additive/Scroll through readback → reference op → upload.
pub(crate) fn apply_frame_raster_op(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    operation: &FrameRasterOp,
) {
    apply_raster_op_pixels(width, height, pixels, operation);
}

fn fill_rect_pixels(width: i32, height: i32, pixels: &mut [u32], rect: FrameRect, color: Color) {
    if rect.is_empty() {
        return;
    }
    let x0 = rect.x.max(0);
    let y0 = rect.y.max(0);
    let x1 = rect.x.saturating_add(rect.width).min(width);
    let y1 = rect.y.saturating_add(rect.height).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let source = color.premultiplied();
    let source_a = source >> 24;
    if source_a == 0 {
        return;
    }
    if source_a == 0xff {
        let row_width = width as usize;
        for y in y0..y1 {
            let start = y as usize * row_width + x0 as usize;
            pixels[start..start + (x1 - x0) as usize].fill(source);
        }
        return;
    }
    for y in y0..y1 {
        for x in x0..x1 {
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_src_over(source, pixels[index]);
        }
    }
}

fn fill_rect_additive_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    clip: FrameRect,
) {
    // 同时裁到命令 clip 与真实 surface，避免任何越界写入。
    let Some(visible) = rect
        .intersection(clip)
        .and_then(|visible| visible.intersection(FrameRect::new(0, 0, width, height)))
    else {
        // 完全不可见时保持累计目标不变。
        return;
    };
    // 可见交集已经是 surface 内的安全半开区间。
    let x0 = visible.x;
    // 保存可见区顶部。
    let y0 = visible.y;
    // 计算可见区右边界。
    let x1 = visible.x + visible.width;
    // 计算可见区底边界。
    let y1 = visible.y + visible.height;
    let source = color.premultiplied();
    for y in y0..y1 {
        for x in x0..x1 {
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_additive(source, pixels[index]);
        }
    }
}

fn fill_rounded_rect_additive_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
    clip: FrameRect,
) {
    // 空几何或空裁剪都不产生覆盖。
    if rect.is_empty() || clip.is_empty() {
        // 保持累计目标不变。
        return;
    }
    // 创建与目标尺寸一致的参考软件光栅器。
    let mut renderer = SoftwareRasterizer::new(width, height);
    // 把 FrameEncoder 的整数 clip 注入软件光栅状态。
    renderer.push_clip_surface(Rect::new(
        clip.x as f32,
        clip.y as f32,
        clip.width as f32,
        clip.height as f32,
    ));
    // 选择目标相关饱和加法语义。
    renderer.set_blend_mode(BlendMode::Additive);
    renderer.fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ),
        color,
        Some(radius.to_radius()),
    );
}

fn fill_rounded_rect_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
) {
    if rect.is_empty() {
        return;
    }
    let renderer = SoftwareRasterizer::new(width, height);
    renderer.fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ),
        color,
        Some(radius.to_radius()),
    );
}

fn fill_rounded_rect_pixels_clipped(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    radius: FrameRadius,
    clip: FrameRect,
) {
    if rect.is_empty() || clip.is_empty() {
        return;
    }
    let mut renderer = SoftwareRasterizer::new(width, height);
    renderer.push_clip_surface(Rect::new(
        clip.x as f32,
        clip.y as f32,
        clip.width as f32,
        clip.height as f32,
    ));
    renderer.fill_rect(
        pixels,
        width,
        height,
        Rect::new(
            rect.x as f32,
            rect.y as f32,
            rect.width as f32,
            rect.height as f32,
        ),
        color,
        Some(radius.to_radius()),
    );
}

fn scroll_copy_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    viewport: FrameRect,
    dx: i32,
    dy: i32,
) {
    if viewport.is_empty() || (dx == 0 && dy == 0) {
        return;
    }
    // Match SharedRasterizer::scroll_region: source is viewport shifted by
    // (dx, dy), destination is the viewport origin.
    let src = FrameRect::new(
        viewport.x.saturating_add(dx),
        viewport.y.saturating_add(dy),
        viewport.width,
        viewport.height,
    );
    let src_x = src.x;
    let src_y = src.y;
    let copy_w = src.width;
    let copy_h = src.height;
    let dst_x = viewport.x;
    let dst_y = viewport.y;

    let clip_x0 = src_x.max(0).max(src_x - dst_x);
    let clip_y0 = src_y.max(0).max(src_y - dst_y);
    let clip_x1 = (src_x + copy_w).min(width).min(width + src_x - dst_x);
    let clip_y1 = (src_y + copy_h).min(height).min(height + src_y - dst_y);
    if clip_x0 >= clip_x1 || clip_y0 >= clip_y1 {
        return;
    }
    let row_len = (clip_x1 - clip_x0) as usize;
    if dst_y <= src_y {
        for row in clip_y0..clip_y1 {
            let src_idx = (row * width + clip_x0) as usize;
            let dst_idx = ((row + dst_y - src_y) * width + (clip_x0 + dst_x - src_x)) as usize;
            pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
        }
    } else {
        for row in (clip_y0..clip_y1).rev() {
            let src_idx = (row * width + clip_x0) as usize;
            let dst_idx = ((row + dst_y - src_y) * width + (clip_x0 + dst_x - src_x)) as usize;
            pixels.copy_within(src_idx..src_idx + row_len, dst_idx);
        }
    }
}

pub(super) fn blit_image_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
) {
    blit_image_pixels_impl::<false>(width, height, pixels, image, src, dst, 1.0);
}

pub(super) fn blit_image_pixels_with_opacity(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    blit_image_pixels_with_opacity_blend(width, height, pixels, image, src, dst, opacity, false);
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors one encoded image blit operation"
)]
pub(super) fn blit_image_pixels_with_opacity_blend(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
    additive: bool,
) {
    if additive {
        blit_image_pixels_additive(width, height, pixels, image, src, dst, opacity);
        return;
    }
    if opacity >= 1.0 - 1e-6 {
        blit_image_pixels_impl::<false>(width, height, pixels, image, src, dst, 1.0);
    } else {
        blit_image_pixels_impl::<true>(width, height, pixels, image, src, dst, opacity);
    }
}

/// 亚像素 / 缩放 Picture 目标的 CPU 参考采样（最近邻），与 GPU 纹理四边形落点对齐。
pub(super) fn blit_sampled_image_pixels_with_opacity(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameSampledRect,
    opacity: f32,
) {
    blit_sampled_image_pixels_with_opacity_blend(
        width, height, pixels, image, src, dst, opacity, false,
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "the helper mirrors one encoded sampled image blit operation"
)]
pub(super) fn blit_sampled_image_pixels_with_opacity_blend(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameSampledRect,
    opacity: f32,
    additive: bool,
) {
    if src.is_empty() || dst.width() <= 0.0 || dst.height() <= 0.0 {
        return;
    }
    let apply_opacity = opacity < 1.0 - 1e-6;
    let x0 = dst.x().floor().max(0.0) as i32;
    let y0 = dst.y().floor().max(0.0) as i32;
    let x1 = (dst.x() + dst.width()).ceil().min(width as f32) as i32;
    let y1 = (dst.y() + dst.height()).ceil().min(height as f32) as i32;
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    let inv_w = 1.0 / dst.width();
    let inv_h = 1.0 / dst.height();
    for y in y0..y1 {
        for x in x0..x1 {
            let u = ((x as f32 + 0.5) - dst.x()) * inv_w;
            let v = ((y as f32 + 0.5) - dst.y()) * inv_h;
            if u < 0.0 || v < 0.0 || u >= 1.0 || v >= 1.0 {
                continue;
            }
            let source_x = src.x + (u * src.width as f32).floor() as i32;
            let source_y = src.y + (v * src.height as f32).floor() as i32;
            if source_x < 0
                || source_y < 0
                || source_x >= image.width
                || source_y >= image.height
                || source_x >= src.x + src.width
                || source_y >= src.y + src.height
            {
                continue;
            }
            let mut source =
                image.pixels[source_y as usize * image.width as usize + source_x as usize];
            if apply_opacity {
                source = crate::draw::raster::rasterizer::apply_opacity(source, opacity);
            }
            let index = y as usize * width as usize + x as usize;
            pixels[index] = if additive {
                blend_pixel_additive(source, pixels[index])
            } else {
                blend_pixel_src_over(source, pixels[index])
            };
        }
    }
}

fn blit_image_pixels_additive(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    if src.is_empty() || dst.is_empty() {
        return;
    }
    let apply_opacity = opacity < 1.0 - 1e-6;
    let x0 = dst.x.max(0);
    let y0 = dst.y.max(0);
    let x1 = dst.x.saturating_add(dst.width).min(width);
    let y1 = dst.y.saturating_add(dst.height).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }
    // Additive 参考路径只保证 1:1；缩放走采样路径。
    if src.width != dst.width || src.height != dst.height {
        blit_sampled_image_pixels_with_opacity_blend(
            width,
            height,
            pixels,
            image,
            src,
            FrameSampledRect::from_integer(dst),
            opacity,
            true,
        );
        return;
    }
    for y in y0..y1 {
        let source_y = src.y + (y - dst.y);
        for x in x0..x1 {
            let source_x = src.x + (x - dst.x);
            if source_x < src.x
                || source_y < src.y
                || source_x >= src.x + src.width
                || source_y >= src.y + src.height
            {
                continue;
            }
            let mut source =
                image.pixels[source_y as usize * image.width as usize + source_x as usize];
            if apply_opacity {
                source = crate::draw::raster::rasterizer::apply_opacity(source, opacity);
            }
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_additive(source, pixels[index]);
        }
    }
}

fn blit_image_pixels_impl<const APPLY_OPACITY: bool>(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    if src.is_empty() || dst.is_empty() {
        return;
    }
    if src.width == dst.width && src.height == dst.height {
        blit_unscaled_image_pixels::<APPLY_OPACITY>(
            width, height, pixels, image, src, dst, opacity,
        );
        return;
    }
    let x0 = dst.x.max(0);
    let y0 = dst.y.max(0);
    let x1 = dst.x.saturating_add(dst.width).min(width);
    let y1 = dst.y.saturating_add(dst.height).min(height);
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    for y in y0..y1 {
        for x in x0..x1 {
            let local_x = x - dst.x;
            let local_y = y - dst.y;
            let source_x = src.x + local_x.saturating_mul(src.width) / dst.width;
            let source_y = src.y + local_y.saturating_mul(src.height) / dst.height;
            if source_x < 0 || source_y < 0 || source_x >= image.width || source_y >= image.height {
                continue;
            }
            let source = image.pixels[source_y as usize * image.width as usize + source_x as usize];
            let source = if APPLY_OPACITY {
                crate::draw::raster::rasterizer::apply_opacity(source, opacity)
            } else {
                source
            };
            let index = y as usize * width as usize + x as usize;
            pixels[index] = blend_pixel_src_over(source, pixels[index]);
        }
    }
}

fn blit_unscaled_image_pixels<const APPLY_OPACITY: bool>(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    image: &FrameImage,
    src: FrameRect,
    dst: FrameRect,
    opacity: f32,
) {
    // CPU segment 与大多数 Picture blit 都是同尺寸搬运；先同时裁目标与源，
    // 再按连续行处理，避免热路径逐像素整数除法与边界判断。
    let x0 = dst.x.max(0).max(dst.x.saturating_sub(src.x));
    let y0 = dst.y.max(0).max(dst.y.saturating_sub(src.y));
    let x1 = dst
        .x
        .saturating_add(dst.width)
        .min(width)
        .min(dst.x.saturating_add(image.width).saturating_sub(src.x));
    let y1 = dst
        .y
        .saturating_add(dst.height)
        .min(height)
        .min(dst.y.saturating_add(image.height).saturating_sub(src.y));
    if x0 >= x1 || y0 >= y1 {
        return;
    }

    let copy_width = (x1 - x0) as usize;
    for y in y0..y1 {
        let source_x = src.x + x0 - dst.x;
        let source_y = src.y + y - dst.y;
        let source_start = source_y as usize * image.width as usize + source_x as usize;
        let destination_start = y as usize * width as usize + x0 as usize;
        let source_row = &image.pixels[source_start..source_start + copy_width];
        let destination_row = &mut pixels[destination_start..destination_start + copy_width];
        for (&source, destination) in source_row.iter().zip(destination_row) {
            let source = if APPLY_OPACITY {
                crate::draw::raster::rasterizer::apply_opacity(source, opacity)
            } else {
                source
            };
            match source >> 24 {
                0 => {}
                0xff => *destination = source,
                _ => *destination = blend_pixel_src_over(source, *destination),
            }
        }
    }
}

fn blend_pixel_src_over(source: u32, destination: u32) -> u32 {
    let source_a = (source >> 24) & 0xff;
    if source_a == 0 {
        return destination;
    }
    if source_a == 0xff {
        return source;
    }
    let destination_a = (destination >> 24) & 0xff;
    crate::draw::raster::rasterizer::core::blend_srcover(
        source_a,
        destination_a,
        (source >> 16) & 0xff,
        (source >> 8) & 0xff,
        source & 0xff,
        (destination >> 16) & 0xff,
        (destination >> 8) & 0xff,
        destination & 0xff,
    )
}

fn blend_pixel_additive(source: u32, destination: u32) -> u32 {
    let source_a = (source >> 24) & 0xff;
    if source_a == 0 {
        return destination;
    }
    let add = |shift: u32| (((source >> shift) & 0xff) + ((destination >> shift) & 0xff)).min(0xff);
    (add(24) << 24) | (add(16) << 16) | (add(8) << 8) | add(0)
}
