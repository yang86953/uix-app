//! 帧执行器复用的矩形描边纯函数。

use crate::core::Rect;

use super::{
    clip_to_int, color_to_premul, fill_rect_raw, intersect_rect, put_pixel_aa, rounded_rect_sdf,
    sdf_to_coverage,
};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::Radius;

/// 纯函数：描边矩形，可选圆角。
pub fn stroke_rect(
    pixels: &mut [u32],
    surface_w: i32,
    surface_h: i32,
    clip: Rect,
    opacity: f32,
    rect: Rect,
    color: Color,
    line_width: f32,
    radius: Option<Radius>,
) {
    let lw = line_width.max(0.0);
    let rad = radius.unwrap_or_default();
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);

    // 快速路径：整数坐标、1px 描边、无圆角
    if rad.tl == 0.0
        && rad.tr == 0.0
        && rad.bl == 0.0
        && rad.br == 0.0
        && rect.x.fract() == 0.0
        && rect.y.fract() == 0.0
        && rect.w.fract() == 0.0
        && rect.h.fract() == 0.0
        && lw == 1.0
        && lw.fract() == 0.0
    {
        let x0 = rect.x as i32;
        let y0 = rect.y as i32;
        let w = rect.w as i32;
        let h = rect.h as i32;
        let iw = lw as i32;
        if iw * 2 >= w || iw * 2 >= h {
            fill_rect_raw(
                pixels, surface_w, x0, y0, w, h, 0, 0, surface_w, surface_h, c,
            );
            return;
        }
        let inner_h = h - iw * 2;
        let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
        fill_rect_raw(pixels, surface_w, x0, y0, w, iw, cx0, cy0, cx1, cy1, c);
        fill_rect_raw(
            pixels,
            surface_w,
            x0,
            y0 + h - iw,
            w,
            iw,
            cx0,
            cy0,
            cx1,
            cy1,
            c,
        );
        fill_rect_raw(
            pixels,
            surface_w,
            x0,
            y0 + iw,
            iw,
            inner_h,
            cx0,
            cy0,
            cx1,
            cy1,
            c,
        );
        fill_rect_raw(
            pixels,
            surface_w,
            x0 + w - iw,
            y0 + iw,
            iw,
            inner_h,
            cx0,
            cy0,
            cx1,
            cy1,
            c,
        );
        return;
    }

    // SDF 描边
    let h = lw * 0.5;
    let expand = h + 1.0;
    let expanded = Rect::new(
        rect.x - expand,
        rect.y - expand,
        rect.w + expand * 2.0,
        rect.h + expand * 2.0,
    );
    if let Some(cr) = intersect_rect(&expanded, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
        let split = ((rect.x + rect.w * 0.5 - 0.5).ceil() as i32).clamp(x0, x1);
        let optimized = [rad.tl, rad.tr, rad.br, rad.bl]
            .iter()
            .all(|radius| radius.is_finite() && *radius >= 0.0);
        for py in y0..y1 {
            if !optimized {
                for px in x0..x1 {
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                    let coverage = sdf_to_coverage(sd.abs() - h);
                    if coverage > 0.0 {
                        put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                    }
                }
                continue;
            }

            let uy = py as f32 + 0.5;
            let mut saw_coverage = false;
            for px in x0..split {
                let ux = px as f32 + 0.5;
                let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                let coverage = sdf_to_coverage(sd.abs() - h);
                if coverage > 0.0 {
                    saw_coverage = true;
                    put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                } else if saw_coverage {
                    break;
                }
            }

            let mut saw_coverage = false;
            for px in (split..x1).rev() {
                let ux = px as f32 + 0.5;
                let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                let coverage = sdf_to_coverage(sd.abs() - h);
                if coverage > 0.0 {
                    saw_coverage = true;
                    put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                } else if saw_coverage {
                    break;
                }
            }
        }
    }
}
