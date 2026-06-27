//! 光栅化核心工具——所有纯函数工具，rasterizer 子模块和 Canvas2D 实现共用。
//!
//! 消除 rasterizer/mod.rs ↔ engine/cpu/canvas_2d.rs 之间的代码重复。

use uix_platform::Rect;

use crate::types::Radius;

/// 预乘 RGBA 颜色。
#[inline]
pub fn premul(c: u32) -> u32 {
    let a = (c >> 24) & 0xFF;
    if a == 0xFF {
        return c;
    }
    let r = (((c >> 16) & 0xFF) * a / 255).min(255);
    let g = (((c >> 8) & 0xFF) * a / 255).min(255);
    let b = ((c & 0xFF) * a / 255).min(255);
    (a << 24) | (r << 16) | (g << 8) | b
}

/// 合成 α 通道（SrcOver）。
#[inline]
pub fn blend_srcover(src_a: u32, dst_a: u32, src_r: u32, src_g: u32, src_b: u32,
                     dst_r: u32, dst_g: u32, dst_b: u32) -> u32 {
    let out_a = src_a + dst_a - (src_a * dst_a / 255);
    if out_a == 0 {
        return 0;
    }
    let out_r = src_r + (dst_r * (255 - src_a) / 255);
    let out_g = src_g + (dst_g * (255 - src_a) / 255);
    let out_b = src_b + (dst_b * (255 - src_a) / 255);
    (out_a.min(255) << 24)
        | (out_r.min(255) << 16)
        | (out_g.min(255) << 8)
        | out_b.min(255)
}

/// 对 premul 颜色应用透明度系数。
#[inline]
pub fn apply_opacity(color: u32, opacity: f32) -> u32 {
    if opacity >= 1.0 - 1e-6 {
        return color;
    }
    let a = ((color >> 24) & 0xFF) as f32 * opacity;
    let r = ((color >> 16) & 0xFF) as f32 * opacity;
    let g = ((color >> 8) & 0xFF) as f32 * opacity;
    let b = (color & 0xFF) as f32 * opacity;
    ((a as u32).min(255) << 24)
        | ((r as u32).min(255) << 16)
        | ((g as u32).min(255) << 8)
        | (b as u32).min(255)
}

/// 将 Color 转为预乘 u32，并应用透明度。
#[inline]
pub fn color_to_premul(r: u8, g: u8, b: u8, a: u8, opacity: f32) -> u32 {
    let a = (a as f32 * opacity) as u8;
    let ra = a as u32;
    if ra == 0 {
        return 0;
    }
    let r = (r as u32 * ra / 255).min(255);
    let g = (g as u32 * ra / 255).min(255);
    let b = (b as u32 * ra / 255).min(255);
    (ra << 24) | (r << 16) | (g << 8) | b
}

/// 向像素缓冲写入一个像素（SrcOver 混合，考虑裁剪）。
#[inline]
pub fn put_pixel(pixels: &mut [u32], stride: i32, x: i32, y: i32,
                 clip_x0: i32, clip_y0: i32, clip_x1: i32, clip_y1: i32,
                 color: u32) {
    if x < clip_x0 || y < clip_y0 || x >= clip_x1 || y >= clip_y1 {
        return;
    }
    let idx = (y * stride + x) as usize;
    if idx >= pixels.len() {
        return;
    }
    let src_a = (color >> 24) & 0xFF;
    if src_a == 0 {
        return;
    }
    let dst = pixels[idx];
    let dst_a = (dst >> 24) & 0xFF;
    if src_a == 0xFF && dst_a == 0 {
        pixels[idx] = color;
        return;
    }
    let out = blend_srcover(
        src_a, dst_a,
        (color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF,
        (dst >> 16) & 0xFF, (dst >> 8) & 0xFF, dst & 0xFF,
    );
    pixels[idx] = out;
}

/// `put_pixel` 别名（向下兼容）。
#[inline]
pub fn put_pixel_raw(pixels: &mut [u32], stride: i32, x: i32, y: i32,
                     clip_x0: i32, clip_y0: i32, clip_x1: i32, clip_y1: i32,
                     color: u32) {
    put_pixel(pixels, stride, x, y, clip_x0, clip_y0, clip_x1, clip_y1, color);
}

/// 写入一个带抗锯齿覆盖率的像素。
#[inline]
pub fn put_pixel_aa(pixels: &mut [u32], stride: i32, x: i32, y: i32,
                    clip_x0: i32, clip_y0: i32, clip_x1: i32, clip_y1: i32,
                    premul_color: u32, coverage: f32) {
    if x < clip_x0 || y < clip_y0 || x >= clip_x1 || y >= clip_y1 {
        return;
    }
    if coverage >= 1.0 - 1e-6 {
        put_pixel(pixels, stride, x, y, clip_x0, clip_y0, clip_x1, clip_y1, premul_color);
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

    let idx = (y * stride + x) as usize;
    if idx >= pixels.len() {
        return;
    }
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

/// 填充一整行上的连续区间，针对不透明颜色优化。
#[inline]
pub fn fill_span(pixels: &mut [u32], stride: i32, x0: i32, x1: i32, y: i32,
                 clip_x0: i32, clip_y0: i32, clip_x1: i32, clip_y1: i32,
                 color: u32) {
    let x_start = x0.max(clip_x0);
    let x_end = x1.min(clip_x1);
    if y < clip_y0 || y >= clip_y1 || x_start >= x_end {
        return;
    }
    if (color >> 24) == 0xFF {
        let start = (y * stride + x_start) as usize;
        let len = (x_end - x_start) as usize;
        if start + len <= pixels.len() {
            pixels[start..start + len].fill(color);
        }
    } else {
        for x in x_start..x_end {
            put_pixel(pixels, stride, x, y, clip_x0, clip_y0, clip_x1, clip_y1, color);
        }
    }
}

/// 填充矩形区域。
#[inline]
pub fn fill_rect_raw(pixels: &mut [u32], stride: i32, x: i32, y: i32, w: i32, h: i32,
                     clip_x0: i32, clip_y0: i32, clip_x1: i32, clip_y1: i32,
                     color: u32) {
    for dy in 0..h {
        fill_span(pixels, stride, x, x + w, y + dy, clip_x0, clip_y0, clip_x1, clip_y1, color);
    }
}

/// 对浮点矩形取整为像素矩形。
#[inline]
pub fn rect_to_pixels(r: &Rect) -> (i32, i32, i32, i32) {
    let x0 = (r.x + 0.5).floor() as i32;
    let y0 = (r.y + 0.5).floor() as i32;
    let x1 = (r.x + r.w + 0.5).floor() as i32;
    let y1 = (r.y + r.h + 0.5).floor() as i32;
    (x0, y0, x1 - x0, y1 - y0)
}

/// 将浮点 clip 转换为整数边界。
#[inline]
pub fn clip_to_int(r: &Rect) -> (i32, i32, i32, i32) {
    (
        (r.x + 0.5).floor() as i32,
        (r.y + 0.5).floor() as i32,
        (r.x + r.w + 0.5).floor() as i32,
        (r.y + r.h + 0.5).floor() as i32,
    )
}

/// 求两个矩形的交集。
#[inline]
pub fn intersect_rect(a: &Rect, b: &Rect) -> Option<Rect> {
    let x = a.x.max(b.x);
    let y = a.y.max(b.y);
    let rgt = (a.x + a.w).min(b.x + b.w);
    let bot = (a.y + a.h).min(b.y + b.h);
    if x < rgt && y < bot {
        Some(Rect::new(x, y, rgt - x, bot - y))
    } else {
        None
    }
}

/// Signed distance field for a rounded rectangle with per-corner radii.
#[inline]
pub fn rounded_rect_sdf(ux: f32, uy: f32, r: &Rect, rad: &Radius) -> f32 {
    if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0 {
        let dx = (r.x - ux).max(ux - (r.x + r.w)).max(0.0);
        let dy = (r.y - uy).max(uy - (r.y + r.h)).max(0.0);
        let outside = (dx * dx + dy * dy).sqrt();
        let inside = (r.x - ux)
            .max(ux - (r.x + r.w))
            .max((r.y - uy).max(uy - (r.y + r.h)));
        return if inside < 0.0 { inside } else { outside };
    }

    let cx = r.x + r.w * 0.5;
    let cy = r.y + r.h * 0.5;
    let half_w = r.w * 0.5;
    let half_h = r.h * 0.5;
    let px = ux - cx;
    let py = uy - cy;

    let cr = if px < 0.0 {
        if py < 0.0 { rad.tl } else { rad.bl }
    } else {
        if py < 0.0 { rad.tr } else { rad.br }
    };

    let qx = px.abs() - half_w + cr;
    let qy = py.abs() - half_h + cr;
    let qx_clamped = qx.max(0.0);
    let qy_clamped = qy.max(0.0);
    let outside = (qx_clamped * qx_clamped + qy_clamped * qy_clamped).sqrt();
    let inside = qx.max(qy).min(0.0);
    outside + inside - cr
}

/// Signed distance from point to a line segment.
#[inline]
pub fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length_sq = dx * dx + dy * dy;
    if length_sq < 1e-12 {
        let dx0 = ux - x1;
        let dy0 = uy - y1;
        return (dx0 * dx0 + dy0 * dy0).sqrt();
    }
    let t = ((ux - x1) * dx + (uy - y1) * dy) / length_sq;
    let t = t.clamp(0.0, 1.0);
    let px = x1 + t * dx;
    let py = y1 + t * dy;
    ((ux - px).powi(2) + (uy - py).powi(2)).sqrt()
}

/// Convert SDF value to pixel coverage.
#[inline]
pub fn sdf_to_coverage(sd: f32) -> f32 {
    ((0.5 - sd) / (2.0 * 0.5)).clamp(0.0, 1.0)
}

/// Convert SDF value to pixel coverage with custom AA half-width.
#[inline]
pub fn sdf_to_coverage_aa(sd: f32, aa_half: f32) -> f32 {
    ((aa_half - sd) / (2.0 * aa_half)).clamp(0.0, 1.0)
}

/// Shadow coverage with smooth Gaussian-like falloff.
#[inline]
pub fn shadow_coverage(sd: f32, blur: f32) -> f32 {
    let t = ((blur - sd) / (2.0 * blur)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Wider, softer shadow falloff for ambient layers.
#[inline]
pub fn shadow_coverage_ambient(sd: f32, blur: f32) -> f32 {
    let half = blur * 0.5;
    let t = ((half - sd) / (blur + half)).clamp(0.0, 1.0);
    let t2 = t * t;
    t2 * t2 * (5.0 - 4.0 * t)
}
