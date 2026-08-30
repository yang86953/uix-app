//! 字形混合纯函数——将 coverage 位图与颜色混合到像素缓冲。

use crate::core::Rect;

use crate::draw::geometry::color::Color;

use super::{
    clip_to_int, color_to_premul, modulate_coverage, put_pixel,
};

/// 纯函数：将 coverage 位图和颜色混合到指定位置。
///
/// `x`, `y` — 绘制的目标位置（像素坐标）。
/// `coverage` — 每个像素的覆盖率（0-255）。
/// `width`, `height` — coverage 缓冲的尺寸。
/// `color` — 字形颜色。
pub(crate) fn blit_glyph(
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
    if surface_w <= 0 || surface_h <= 0 || width == 0 || height == 0 {
        return;
    }
    let Some(coverage_len) = width.checked_mul(height) else {
        return;
    };
    let Some(surface_len) = (surface_w as usize).checked_mul(surface_h as usize) else {
        return;
    };
    if coverage.len() < coverage_len || pixels.len() < surface_len {
        return;
    }

    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    // 与 SharedRasterizer 消费同一 premul 量化规则；输出逐字节一致。
    let premul = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    if premul == 0 {
        return;
    }

    let visible_x0 = (x as i128).max(cx0.max(0) as i128);
    let visible_y0 = (y as i128).max(cy0.max(0) as i128);
    let visible_x1 = (x as i128 + width as i128).min(cx1.min(surface_w) as i128);
    let visible_y1 = (y as i128 + height as i128).min(cy1.min(surface_h) as i128);
    if visible_x0 >= visible_x1 || visible_y0 >= visible_y1 {
        return;
    }

    let col_start = (visible_x0 - x as i128) as usize;
    let col_end = (visible_x1 - x as i128) as usize;
    let row_start = (visible_y0 - y as i128) as usize;
    let row_end = (visible_y1 - y as i128) as usize;
    for row in row_start..row_end {
        let py = (y as i128 + row as i128) as i32;
        let coverage_row = row * width;
        for col in col_start..col_end {
            let px = (x as i128 + col as i128) as i32;
            let cov = coverage[coverage_row + col];
            if cov == 0 {
                continue;
            }
            // Modulate by coverage（共享调制函数）。
            let final_color = modulate_coverage(premul, cov);
            put_pixel(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, final_color);
        }
    }
}
