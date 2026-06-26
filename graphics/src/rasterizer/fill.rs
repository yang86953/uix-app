//! 矢量填充纯函数——矩形/圆/椭圆/扇形/路径。
//!
//! 所有函数不持状态，只接受像素缓冲、裁剪、参数，直接写入像素。

use uix_core::Rect;

use crate::color::Color;
use crate::path::{FillRule, Path};
use crate::types::Radius;

use super::{
    color_to_premul, fill_rect_raw, fill_span, put_pixel_aa, rect_to_pixels,
    rounded_rect_sdf, sdf_to_coverage, clip_to_int, intersect_rect,
};

/// 纯函数：填充矩形，可选圆角。
pub fn fill_rect(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    rect: Rect, color: Color, radius: Option<Radius>,
) {
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);

    if let Some(rad) = radius {
        if rad.tl != 0.0 || rad.tr != 0.0 || rad.bl != 0.0 || rad.br != 0.0 {
            let expanded = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
            if let Some(cr) = intersect_rect(&expanded, &clip) {
                let x0 = cr.x as i32;
                let y0 = cr.y as i32;
                let x1 = (cr.x + cr.w) as i32;
                let y1 = (cr.y + cr.h) as i32;
                for py in y0..y1 {
                    for px in x0..x1 {
                        let ux = px as f32 + 0.5;
                        let uy = py as f32 + 0.5;
                        let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                        let coverage = sdf_to_coverage(sd);
                        if coverage > 0.0 {
                            put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                        }
                    }
                }
            }
            return;
        }
    }

    // Sharp rect (no rounded corners) — fast path
    let rad = radius.unwrap_or_default();
    let is_sharp = rad.tl == 0.0 && rad.tr == 0.0 && rad.br == 0.0 && rad.bl == 0.0;
    if is_sharp {
        let (x0, y0, cw, ch) = rect_to_pixels(&rect);
        if cw > 0 && ch > 0 {
            fill_rect_raw(pixels, surface_w, x0, y0, cw, ch, cx0, cy0, cx1, cy1, c);
        }
        return;
    }

    // Generic SDF rounded rect
    let expand = 1.0;
    let expanded = Rect::new(rect.x - expand, rect.y - expand, rect.w + expand * 2.0, rect.h + expand * 2.0);
    if let Some(cr) = intersect_rect(&expanded, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let ux = px as f32 + 0.5;
                let uy = py as f32 + 0.5;
                let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                let coverage = sdf_to_coverage(sd);
                if coverage > 0.0 {
                    put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                }
            }
        }
    }
}

/// 纯函数：填充圆形。
pub fn fill_circle(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    cx: f32, cy: f32, r: f32, color: Color,
) {
    if r <= 0.0 { return; }
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let expand = r + 1.0;
    let inner = (r - 1.0).max(0.0);
    let inner2 = inner * inner;
    let outer2 = (r + 1.0) * (r + 1.0);

    let x0 = (cx - expand).max(clip.x) as i32;
    let y0 = (cy - expand).max(clip.y) as i32;
    let x1 = (cx + expand).min(clip.x + clip.w) as i32;
    let y1 = (cy + expand).min(clip.y + clip.h) as i32;
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let dist2 = dx * dx + dy * dy;
            if dist2 <= inner2 {
                put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, 1.0);
                continue;
            }
            if dist2 >= outer2 { continue; }
            let dist = dist2.sqrt();
            let coverage = sdf_to_coverage(dist - r);
            if coverage > 0.0 {
                put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
            }
        }
    }
}

/// 纯函数：填充椭圆。
pub fn fill_ellipse(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    rect: Rect, color: Color,
) {
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
    let (rx, ry) = (rect.w / 2.0, rect.h / 2.0);
    if rx <= 0.0 || ry <= 0.0 { return; }
    let inv_rx2 = 1.0 / (rx * rx);
    let inv_ry2 = 1.0 / (ry * ry);
    let expand = 1.0;
    let expanded = Rect::new(rect.x - expand, rect.y - expand, rect.w + expand * 2.0, rect.h + expand * 2.0);
    if let Some(cr) = intersect_rect(&expanded, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        let (cix0, ciy0, cix1, ciy1) = clip_to_int(&clip);
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let tx = dx * dx * inv_rx2;
                let ty = dy * dy * inv_ry2;
                let v = tx + ty;
                if v >= 1.15 { continue; }
                if v <= 0.85 {
                    put_pixel_aa(pixels, surface_w, px, py, cix0, ciy0, cix1, ciy1, c, 1.0);
                    continue;
                }
                let grad_mag = 2.0 * (tx * inv_rx2 + ty * inv_ry2).sqrt();
                let sd = (v - 1.0) / grad_mag.max(1e-12);
                let coverage = sdf_to_coverage(sd);
                if coverage > 0.0 {
                    put_pixel_aa(pixels, surface_w, px, py, cix0, ciy0, cix1, ciy1, c, coverage);
                }
            }
        }
    }
}

/// 纯函数：填充扇形。
pub fn fill_sector(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    cx: f32, cy: f32, r: f32,
    start_angle: f32, end_angle: f32,
    color: Color,
) {
    if r <= 0.0 { return; }
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let expand = r + 1.0;
    let norm = |a: f32| a.rem_euclid(std::f32::consts::TAU);
    let sa = norm(start_angle);
    let ea = norm(end_angle);
    let in_sector = |angle: f32| -> bool {
        let a = norm(angle);
        if sa <= ea { a >= sa && a <= ea } else { a >= sa || a <= ea }
    };
    let outer2 = (r + 1.0) * (r + 1.0);

    let x0 = (cx - expand).max(clip.x) as i32;
    let y0 = (cy - expand).max(clip.y) as i32;
    let x1 = (cx + expand).min(clip.x + clip.w) as i32;
    let y1 = (cy + expand).min(clip.y + clip.h) as i32;
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let dist2 = dx * dx + dy * dy;
            if dist2 >= outer2 { continue; }
            let angle = dy.atan2(dx);
            if !in_sector(angle) { continue; }
            let coverage = sdf_to_coverage(dist2.sqrt() - r);
            if coverage > 0.0 {
                put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
            }
        }
    }
}

/// 纯函数：填充闭合路径（暂未实现，委托 polygon fill）。
pub fn fill_path(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    _path: &Path, color: Color, fill_rule: FillRule,
) {
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    // TODO: Path flatten + polygon fill
    let _ = (pixels, surface_w, surface_h, clip, c, fill_rule);
}
