// 各项自带 cfg(test) 门控，在源文件模块作用域内 include! 展开。

// —— 以下仅测试参考填充实现自 tests-src 归位（依赖本文件私有 pixel 辅助） ——

#[cfg(test)]
#[inline]
pub(crate) fn put_pixel_aa(
    pixels: &mut [u32],
    stride: i32,
    x: i32,
    y: i32,
    clip_x0: i32,
    clip_y0: i32,
    clip_x1: i32,
    clip_y1: i32,
    premul_color: u32,
    coverage: f32,
) {
    if x < clip_x0 || y < clip_y0 || x >= clip_x1 || y >= clip_y1 {
        return;
    }
    if coverage >= 1.0 - 1e-6 {
        put_pixel(
            pixels,
            stride,
            x,
            y,
            clip_x0,
            clip_y0,
            clip_x1,
            clip_y1,
            premul_color,
        );
        return;
    }
    if coverage <= 0.0 {
        return;
    }
    let src_a = ((premul_color >> 24) & 0xFF) as f32;
    if src_a <= 0.0 {
        return;
    }
    let src_r_p = ((premul_color >> 16) & 0xFF) as f32 * coverage;
    let src_g_p = ((premul_color >> 8) & 0xFF) as f32 * coverage;
    let src_b_p = (premul_color & 0xFF) as f32 * coverage;
    let src_a_s = src_a * coverage;

    let Some(idx) = pixel_index(pixels.len(), stride, x, y) else {
        return;
    };
    let dst = pixels[idx];
    let dst_a = ((dst >> 24) & 0xFF) as f32;
    let dst_r_p = ((dst >> 16) & 0xFF) as f32;
    let dst_g_p = ((dst >> 8) & 0xFF) as f32;
    let dst_b_p = (dst & 0xFF) as f32;

    let inv = 1.0 - (src_a_s / 255.0);
    let out_a = src_a_s + dst_a * inv;
    let out_r_p = src_r_p + dst_r_p * inv;
    let out_g_p = src_g_p + dst_g_p * inv;
    let out_b_p = src_b_p + dst_b_p * inv;

    pixels[idx] = ((out_a.round() as u32).min(255) << 24)
        | ((out_r_p.round() as u32).min(255) << 16)
        | ((out_g_p.round() as u32).min(255) << 8)
        | (out_b_p.round() as u32).min(255);
}

#[cfg(test)]
#[inline]
pub(crate) fn fill_rect_raw(
    pixels: &mut [u32],
    stride: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
    clip_x0: i32,
    clip_y0: i32,
    clip_x1: i32,
    clip_y1: i32,
    color: u32,
) {
    if stride <= 0 || w <= 0 || h <= 0 {
        return;
    }
    let surface_height = pixels.len() / stride as usize;
    let x_start = (x as i64).max(clip_x0 as i64).max(0);
    let x_end = (x as i64 + w as i64).min(clip_x1 as i64).min(stride as i64);
    let y_start = (y as i64).max(clip_y0 as i64).max(0);
    let y_end = (y as i64 + h as i64)
        .min(clip_y1 as i64)
        .min(surface_height as i64);
    if x_start >= x_end || y_start >= y_end {
        return;
    }

    for row in y_start..y_end {
        fill_span(
            pixels,
            stride,
            x_start as i32,
            x_end as i32,
            row as i32,
            clip_x0,
            clip_y0,
            clip_x1,
            clip_y1,
            color,
        );
    }
}

#[inline]
#[cfg(test)]
#[doc(hidden)]
pub fn rect_to_pixels(r: &Rect) -> (i32, i32, i32, i32) {
    let x0 = (r.x + 0.5).floor() as i32;
    let y0 = (r.y + 0.5).floor() as i32;
    let x1 = (r.x + r.w + 0.5).floor() as i32;
    let y1 = (r.y + r.h + 0.5).floor() as i32;
    (x0, y0, x1 - x0, y1 - y0)
}
