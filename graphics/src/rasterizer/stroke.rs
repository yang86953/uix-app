//! 矢量描边纯函数——矩形描边/圆形描边/路径描边/直线。

use uix_core::Rect;

use crate::color::Color;
use crate::path::Path;
use crate::stroker::StrokeOptions;
use crate::types::Radius;

use super::{
    color_to_premul, fill_rect_raw, put_pixel_aa,
    rounded_rect_sdf, line_segment_sdf, sdf_to_coverage, clip_to_int, intersect_rect,
};
use super::fill;

/// 纯函数：描边矩形，可选圆角。
pub fn stroke_rect(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    rect: Rect, color: Color, line_width: f32, radius: Option<Radius>,
) {
    let lw = line_width.max(0.0);
    let rad = radius.unwrap_or_default();
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);

    // 快速路径：整数坐标、1px 描边、无圆角
    if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0
        && rect.x.fract() == 0.0 && rect.y.fract() == 0.0
        && rect.w.fract() == 0.0 && rect.h.fract() == 0.0
        && lw == 1.0 && lw.fract() == 0.0
    {
        let x0 = rect.x as i32;
        let y0 = rect.y as i32;
        let w = rect.w as i32;
        let h = rect.h as i32;
        let iw = lw as i32;
        if iw * 2 >= w || iw * 2 >= h {
            fill_rect_raw(pixels, surface_w, x0, y0, w, h, 0, 0, surface_w, surface_h, c);
            return;
        }
        let inner_h = h - iw * 2;
        let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
        fill_rect_raw(pixels, surface_w, x0, y0, w, iw, cx0, cy0, cx1, cy1, c);
        fill_rect_raw(pixels, surface_w, x0, y0 + h - iw, w, iw, cx0, cy0, cx1, cy1, c);
        fill_rect_raw(pixels, surface_w, x0, y0 + iw, iw, inner_h, cx0, cy0, cx1, cy1, c);
        fill_rect_raw(pixels, surface_w, x0 + w - iw, y0 + iw, iw, inner_h, cx0, cy0, cx1, cy1, c);
        return;
    }

    // SDF 描边
    let h = lw * 0.5;
    let expand = h + 1.0;
    let expanded = Rect::new(rect.x - expand, rect.y - expand,
        rect.w + expand * 2.0, rect.h + expand * 2.0);
    if let Some(cr) = intersect_rect(&expanded, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
        for py in y0..y1 {
            for px in x0..x1 {
                let ux = px as f32 + 0.5;
                let uy = py as f32 + 0.5;
                let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                let coverage = sdf_to_coverage(sd.abs() - h);
                if coverage > 0.0 {
                    put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                }
            }
        }
    }
}

/// 纯函数：描边圆形。
pub fn stroke_circle(
    pixels: &mut [u32], surface_w: i32, _surface_h: i32,
    clip: Rect, opacity: f32,
    cx: f32, cy: f32, r: f32, color: Color, line_width: f32,
) {
    let lw = line_width.max(0.0);
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let expand = r + lw * 0.5 + 1.0;
    let x0 = (cx - expand).max(clip.x) as i32;
    let y0 = (cy - expand).max(clip.y) as i32;
    let x1 = (cx + expand).min(clip.x + clip.w) as i32;
    let y1 = (cy + expand).min(clip.y + clip.h) as i32;
    for py in y0..y1 {
        for px in x0..x1 {
            let dx = px as f32 + 0.5 - cx;
            let dy = py as f32 + 0.5 - cy;
            let dist = (dx * dx + dy * dy).sqrt();
            let sd = dist - r;
            let stroke_sd = sd.abs() - lw * 0.5;
            let coverage = sdf_to_coverage(stroke_sd);
            if coverage > 0.0 {
                put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
            }
        }
    }
}

/// 纯函数：描边路径（暂未实现）。
pub fn stroke_path(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    _clip: Rect, _opacity: f32,
    _path: &Path, _color: Color, _opts: &StrokeOptions,
) {
    let _ = (pixels, surface_w, surface_h);
    // TODO: path stroke via stroker + fill
}

/// 纯函数：画直线。
pub fn draw_line(
    pixels: &mut [u32], surface_w: i32, surface_h: i32,
    clip: Rect, opacity: f32,
    x1: f32, y1: f32, x2: f32, y2: f32,
    color: Color, width: f32,
) {
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let half_lw = width.max(0.0) * 0.5;
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);

    // 水平/垂直线快速路径
    if (x1 - x2).abs() < 1e-6 {
        let x = x1 - half_lw;
        let y = y1.min(y2);
        let w = width;
        let h = (y1 - y2).abs();
        let r = Rect::new(x, y, w, h);
        return fill::fill_rect(pixels, surface_w, surface_h, clip, opacity, r, color, None);
    }
    if (y1 - y2).abs() < 1e-6 {
        let x = x1.min(x2);
        let y = y1 - half_lw;
        let w = (x1 - x2).abs();
        let h = width;
        let r = Rect::new(x, y, w, h);
        return fill::fill_rect(pixels, surface_w, surface_h, clip, opacity, r, color, None);
    }

    let expand = half_lw + 1.0;
    let bb = Rect::new(
        x1.min(x2) - expand,
        y1.min(y2) - expand,
        (x1 - x2).abs() + expand * 2.0,
        (y1 - y2).abs() + expand * 2.0,
    );
    if let Some(cr) = intersect_rect(&bb, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1b = (cr.x + cr.w) as i32;
        let y1b = (cr.y + cr.h) as i32;
        for py in y0..y1b {
            for px in x0..x1b {
                let ux = px as f32 + 0.5;
                let uy = py as f32 + 0.5;
                let sd = line_segment_sdf(ux, uy, x1, y1, x2, y2) - half_lw;
                let coverage = sdf_to_coverage(sd);
                if coverage > 0.0 {
                    put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                }
            }
        }
    }
}
