//! 字形混合纯函数——将 coverage 位图与颜色混合到像素缓冲。

use crate::platform::Rect;

use crate::render::color::Color;

use super::{clip_to_int, put_pixel};

/// 纯函数：将 coverage 位图和颜色混合到指定位置。
///
/// `x`, `y` — 绘制的目标位置（像素坐标）。
/// `coverage` — 每个像素的覆盖率（0-255）。
/// `width`, `height` — coverage 缓冲的尺寸。
/// `color` — 字形颜色。
pub fn blit_glyph(
    pixels: &mut [u32],
    surface_w: i32,
    surface_h: i32,
    clip: Rect,
    opacity: f32,
    x: i32,
    y: i32,
    coverage: &[u8],
    width: usize,
    height: usize,
    color: Color,
) {
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let ca = (color.a as f32 * opacity) as u8;
    if ca == 0 {
        return;
    }
    let premul = if ca == color.a {
        super::premul(color.to_rgba())
    } else {
        let r = (color.r as u32 * ca as u32 / 255).min(255) as u8;
        let g = (color.g as u32 * ca as u32 / 255).min(255) as u8;
        let b = (color.b as u32 * ca as u32 / 255).min(255) as u8;
        (ca as u32) << 24 | (r as u32) << 16 | (g as u32) << 8 | b as u32
    };

    for row in 0..height {
        let py = y + row as i32;
        if py < cy0.max(0) || py >= cy1.min(surface_h) {
            continue;
        }
        for col in 0..width {
            let px = x + col as i32;
            if px < cx0.max(0) || px >= cx1.min(surface_w) {
                continue;
            }
            let cov = coverage[row * width + col];
            if cov == 0 {
                continue;
            }
            // Modulate by coverage
            let alpha = ((premul >> 24) & 0xFF) * cov as u32 / 255;
            let r = ((premul >> 16) & 0xFF) * cov as u32 / 255;
            let g = ((premul >> 8) & 0xFF) * cov as u32 / 255;
            let b = (premul & 0xFF) * cov as u32 / 255;
            let final_color =
                (alpha.min(255) << 24) | (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
            put_pixel(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, final_color);
        }
    }
}
