//! CPU 参考执行与像素级光栅操作 — encoder 子模块。
//!
//! 参考执行器（[`super::FrameEncoder::render_reference`]）与 CPU backend /
//! CPU 参考执行与紧边界 source 构造共用的像素填充、blit 与 blend 原语。

use crate::core::Rect;
use crate::draw::Color;
use crate::draw::geometry::types::BlendMode;
use crate::draw::raster::software_rasterizer::SoftwareRasterizer;

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
    // 两种混合模式共用 SoftwareRasterizer 的同一描边实现与真实 blend 规则，
    // 保证 SrcOver 与 Additive 的 AA 语义不漂移。
    let mut renderer = SoftwareRasterizer::new(width, height);
    if additive {
        // 选择逐通道饱和加法混合。
        renderer.set_blend_mode(BlendMode::Additive);
    }
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
        // 亚像素填充复用共享软件 SDF，保持参考执行与 GPU shape 契约一致。
        FrameRasterOp::FillRoundedRectSubpixel {
            rect,
            color,
            radius,
            clip,
        } => apply_subpixel_rect_pixels(
            // 传递真实 surface 尺寸。
            (width, height),
            // 直接修改当前累计目标。
            pixels,
            // 保留未取整的矩形边界。
            rect.to_rect(),
            // 保留调用方颜色。
            *color,
            // 保留共享圆角值。
            *radius,
            // 填充没有描边线宽。
            None,
            // 应用命令携带的整数裁剪。
            *clip,
        ),
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
        // 字形轮廓在参考执行时由同一字体算法生成面积 coverage。
        FrameRasterOp::BlitGlyphOutlines { glyphs, clip } => {
            // 把整数命令裁剪转换为共享软件裁剪。
            let clip = Rect::new(
                // 保存左边界。
                clip.x as f32,
                // 保存上边界。
                clip.y as f32,
                // 保存裁剪宽度。
                clip.width as f32,
                // 保存裁剪高度。
                clip.height as f32,
            );
            // 保持字形原始 painter order。
            for glyph in glyphs {
                // 从轮廓边列表生成参考面积 coverage。
                let Some(coverage) =
                    crate::draw::resources::font::glyph_outline::coverage_from_edges(
                        // 借用共享轮廓边列表。
                        glyph.edges().as_ref(),
                        // 使用命令登记的目标宽度。
                        glyph.width() as usize,
                        // 使用命令登记的目标高度。
                        glyph.height() as usize,
                    )
                else {
                    // 异常轮廓不产生参考像素。
                    continue;
                };
                // 复用稳定的 coverage 光栅语义。
                crate::draw::raster::rasterizer::glyph::blit_glyph(
                    // 直接修改当前累计目标。
                    pixels,
                    // 传递真实目标宽度。
                    width,
                    // 传递真实目标高度。
                    height,
                    // 应用命令裁剪。
                    clip,
                    // 命令颜色已经包含画布 opacity。
                    1.0,
                    // 保存字形水平位置。
                    glyph.x(),
                    // 保存字形垂直位置。
                    glyph.y(),
                    // 借用新生成的面积 coverage。
                    coverage.as_ref(),
                    // 保存目标字形宽度。
                    glyph.width() as usize,
                    // 保存目标字形高度。
                    glyph.height() as usize,
                    // 保存字形颜色。
                    glyph.color(),
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
        // 亚像素描边同样复用共享软件 SDF 作为参考事实。
        FrameRasterOp::StrokeRoundedRectSubpixel {
            rect,
            color,
            radius,
            line_width,
            clip,
        } => apply_subpixel_rect_pixels(
            // 传递真实 surface 尺寸。
            (width, height),
            // 直接修改当前累计目标。
            pixels,
            // 保留未取整的矩形边界。
            rect.to_rect(),
            // 保留调用方颜色。
            *color,
            // 保留共享圆角值。
            *radius,
            // 描边使用已经验证的正有限线宽。
            Some(line_width.value()),
            // 应用命令携带的整数裁剪。
            *clip,
        ),
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

// 在 CPU 参考目标上执行一条亚像素填充或描边命令。
fn apply_subpixel_rect_pixels(
    // 合并宽高参数，避免参考辅助函数参数继续膨胀。
    extent: (i32, i32),
    // 接收当前累计目标像素。
    pixels: &mut [u32],
    // 接收未经取整的真实矩形。
    rect: Rect,
    // 接收直通颜色。
    color: Color,
    // 接收四角半径。
    radius: FrameRadius,
    // None 表示填充，Some 表示描边线宽。
    line_width: Option<f32>,
    // 接收整数 surface 裁剪。
    clip: FrameRect,
) {
    // 解构真实目标宽高。
    let (width, height) = extent;
    // 创建与目标尺寸一致的共享软件光栅器。
    let mut renderer = SoftwareRasterizer::new(width, height);
    // 把命令裁剪注入 surface 空间。
    renderer.push_clip_surface(Rect::new(
        // 保留裁剪左边界。
        clip.x as f32,
        // 保留裁剪上边界。
        clip.y as f32,
        // 保留裁剪宽度。
        clip.width as f32,
        // 保留裁剪高度。
        clip.height as f32,
    ));
    // 根据线宽选择描边或填充，同时保持同一 SDF 实现。
    if let Some(line_width) = line_width {
        // 在累计目标上执行普通 SrcOver 亚像素描边。
        renderer.stroke_rect(
            pixels,
            width,
            height,
            rect,
            color,
            line_width,
            Some(radius.to_radius()),
        );
    } else {
        // 在累计目标上执行普通 SrcOver 亚像素填充。
        renderer.fill_rect(pixels, width, height, rect, color, Some(radius.to_radius()));
    }
}

fn fill_rect_pixels(width: i32, height: i32, pixels: &mut [u32], rect: FrameRect, color: Color) {
    if rect.is_empty() {
        return;
    }
    // 与其余 fill 系列一致，直角填充复用 SoftwareRasterizer 的唯一光栅实现。
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
        None,
    );
}

fn fill_rect_additive_pixels(
    width: i32,
    height: i32,
    pixels: &mut [u32],
    rect: FrameRect,
    color: Color,
    clip: FrameRect,
) {
    // 空几何或空裁剪都不产生覆盖。
    if rect.is_empty() || clip.is_empty() {
        // 保持累计目标不变。
        return;
    }
    // 与 fill_rounded_rect_additive_pixels 同构：软件光栅状态注入整数 clip，
    // 选择目标相关饱和加法语义，复用唯一光栅实现。
    let mut renderer = SoftwareRasterizer::new(width, height);
    renderer.push_clip_surface(Rect::new(
        clip.x as f32,
        clip.y as f32,
        clip.width as f32,
        clip.height as f32,
    ));
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
        None,
    );
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
    let renderer = SoftwareRasterizer::new_with_surface_clip(
        width,
        height,
        Rect::new(
            clip.x as f32,
            clip.y as f32,
            clip.width as f32,
            clip.height as f32,
        ),
    );
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
