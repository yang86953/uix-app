//! 图像混合纯函数——将 src 像素区域缩放绘制到 dst 区域。

use uix_platform::Rect;

use super::{clip_to_int, put_pixel};

/// 纯函数：将 src 矩形区域缩放绘制到 dst 矩形区域。
///
/// `src` — 源图像像素缓冲（BGRA premultiplied）。
/// `src_w` — 源图像宽度（像素）。
/// `src_rect` — 源图像中要截取的矩形区域。
/// `dst_rect` — 目标区域。
pub fn blit_image(
    pixels: &mut [u32], surface_w: i32, _surface_h: i32,
    clip: Rect, opacity: f32,
    src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect,
) {
    if src_w <= 0 || src.is_empty() { return; }
    let src_h = (src.len() / src_w as usize) as i32;
    if src_h <= 0 { return; }

    let sx = src_rect.x.max(0.0) as i32;
    let sy = src_rect.y.max(0.0) as i32;
    let sw = (src_rect.w as i32).min(src_w - sx);
    let sh = (src_rect.h as i32).min(src_h - sy);
    if sw <= 0 || sh <= 0 { return; }

    let dx = dst_rect.x as i32;
    let dy = dst_rect.y as i32;
    let dw = dst_rect.w as i32;
    let dh = dst_rect.h as i32;
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let scaled = dw != sw || dh != sh;

    for row in 0..dh.max(sh) {
        for col in 0..dw.max(sw) {
            let (src_x, src_y) = if scaled {
                (sx + (col * sw / dw), sy + (row * sh / dh))
            } else {
                (sx + col, sy + row)
            };
            let src_idx = (src_y * src_w + src_x) as usize;
            if src_idx >= src.len() { continue; }
            let p = if opacity < 1.0 - 1e-6 {
                super::apply_opacity(src[src_idx], opacity)
            } else {
                src[src_idx]
            };
            if scaled {
                if row < dh && col < dw {
                    put_pixel(pixels, surface_w, dx + col, dy + row, cx0, cy0, cx1, cy1, p);
                }
            } else if row < sh && col < sw {
                put_pixel(pixels, surface_w, dx + col, dy + row, cx0, cy0, cx1, cy1, p);
            }
        }
    }
}
