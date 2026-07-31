//! 图像混合纯函数——将 src 像素区域缩放绘制到 dst 区域。

use crate::core::Rect;

use super::{clip_to_int, put_pixel};

/// 纯函数：将 src 矩形区域缩放绘制到 dst 矩形区域。
///
/// `src` — 源图像像素缓冲（BGRA premultiplied）。
/// `src_w` — 源图像宽度（像素）。
/// `src_rect` — 源图像中要截取的矩形区域。
/// `dst_rect` — 目标区域。
pub fn blit_image(
    pixels: &mut [u32],
    surface_w: i32,
    surface_h: i32,
    clip: Rect,
    opacity: f32,
    src: &[u32],
    src_w: i32,
    src_rect: Rect,
    dst_rect: Rect,
) {
    if surface_w <= 0
        || surface_h <= 0
        || src_w <= 0
        || src.is_empty()
        || !src_rect.x.is_finite()
        || !src_rect.y.is_finite()
        || !src_rect.w.is_finite()
        || !src_rect.h.is_finite()
        || !dst_rect.x.is_finite()
        || !dst_rect.y.is_finite()
        || !dst_rect.w.is_finite()
        || !dst_rect.h.is_finite()
        || src_rect.w <= 0.0
        || src_rect.h <= 0.0
        || dst_rect.w <= 0.0
        || dst_rect.h <= 0.0
    {
        return;
    }
    let Some(surface_len) = (surface_w as usize).checked_mul(surface_h as usize) else {
        return;
    };
    if pixels.len() < surface_len {
        return;
    }

    let src_w = src_w as usize;
    let src_h = src.len() / src_w;
    if src_h == 0 {
        return;
    }

    let sx0 = (src_rect.x as f64).clamp(0.0, src_w as f64) as usize;
    let sy0 = (src_rect.y as f64).clamp(0.0, src_h as f64) as usize;
    let sx1 = (src_rect.x as f64 + src_rect.w as f64).clamp(0.0, src_w as f64) as usize;
    let sy1 = (src_rect.y as f64 + src_rect.h as f64).clamp(0.0, src_h as f64) as usize;
    if sx0 >= sx1 || sy0 >= sy1 {
        return;
    }
    let source_width = sx1 - sx0;
    let source_height = sy1 - sy0;

    let dx = (dst_rect.x as i32) as i64;
    let dy = (dst_rect.y as i32) as i64;
    let dw = (dst_rect.w as i32) as i64;
    let dh = (dst_rect.h as i32) as i64;
    if dw <= 0 || dh <= 0 {
        return;
    }
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let visible_x0 = dx.max(cx0.max(0) as i64);
    let visible_y0 = dy.max(cy0.max(0) as i64);
    let visible_x1 = (dx + dw).min(cx1.min(surface_w) as i64);
    let visible_y1 = (dy + dh).min(cy1.min(surface_h) as i64);
    if visible_x0 >= visible_x1 || visible_y0 >= visible_y1 {
        return;
    }

    for target_y in visible_y0..visible_y1 {
        let row = (target_y - dy) as u128;
        let source_y = sy0 + (row * source_height as u128 / dh as u128) as usize;
        let source_row = source_y * src_w;
        for target_x in visible_x0..visible_x1 {
            let col = (target_x - dx) as u128;
            let source_x = sx0 + (col * source_width as u128 / dw as u128) as usize;
            let source_pixel = src[source_row + source_x];
            let p = if opacity < 1.0 - 1e-6 {
                super::apply_opacity(source_pixel, opacity)
            } else {
                source_pixel
            };
            put_pixel(
                pixels,
                surface_w,
                target_x as i32,
                target_y as i32,
                cx0,
                cy0,
                cx1,
                cy1,
                p,
            );
        }
    }
}
