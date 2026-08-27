//! 测试参考用的矩形填充纯函数。
//!
//! 所有函数不持状态，只接受像素缓冲、裁剪、参数，直接写入像素。

use crate::core::Rect;

use crate::draw::geometry::color::Color;
use crate::draw::geometry::types::Radius;

use super::{
    align_rounded_rect, clip_to_int, color_to_premul, fill_rect_raw, fill_span, intersect_rect,
    put_pixel_aa, rect_to_pixels, rounded_rect_sdf, sdf_to_coverage,
};

/// 纯函数：填充矩形，可选圆角。
pub fn fill_rect(
    pixels: &mut [u32],
    surface_w: i32,
    _surface_h: i32,
    clip: Rect,
    opacity: f32,
    rect: Rect,
    color: Color,
    radius: Option<Radius>,
) {
    let c = color_to_premul(color.r, color.g, color.b, color.a, opacity);
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);

    if let Some(rad) = radius {
        if rad.tl != 0.0 || rad.tr != 0.0 || rad.bl != 0.0 || rad.br != 0.0 {
            // 圆角矩形对齐物理像素网格：亚像素坐标下 SDF 弧线端点与像素中心
            // 错位，导致四角取整不对称（顶/底圆角视觉半径不一致）。
            let Some(rect) = align_rounded_rect(rect) else {
                return;
            };
            let expanded = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
            if let Some(cr) = intersect_rect(&expanded, &clip) {
                let x0 = cr.x as i32;
                let y0 = cr.y as i32;
                let x1 = (cr.x + cr.w) as i32;
                let y1 = (cr.y + cr.h) as i32;
                let split = ((rect.x + rect.w * 0.5 - 0.5).ceil() as i32).clamp(x0, x1);
                let optimized = [rad.tl, rad.tr, rad.br, rad.bl]
                    .iter()
                    .all(|radius| radius.is_finite() && *radius >= 0.0);
                for py in y0..y1 {
                    if !optimized {
                        for px in x0..x1 {
                            let ux = px as f32 + 0.5;
                            let uy = py as f32 + 0.5;
                            let coverage = sdf_to_coverage(rounded_rect_sdf(ux, uy, &rect, &rad));
                            if coverage > 0.0 {
                                put_pixel_aa(
                                    pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage,
                                );
                            }
                        }
                        continue;
                    }

                    let uy = py as f32 + 0.5;
                    let mut px = x0;
                    while px < split {
                        let ux = px as f32 + 0.5;
                        let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                        let coverage = sdf_to_coverage(sd);
                        if coverage >= 1.0 - 1e-6 {
                            fill_span(pixels, surface_w, px, split, py, cx0, cy0, cx1, cy1, c);
                            break;
                        }
                        if coverage > 0.0 {
                            put_pixel_aa(
                                pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage,
                            );
                        }
                        px += 1;
                    }

                    let mut px = x1;
                    while px > split {
                        px -= 1;
                        let ux = px as f32 + 0.5;
                        let sd = rounded_rect_sdf(ux, uy, &rect, &rad);
                        let coverage = sdf_to_coverage(sd);
                        if coverage >= 1.0 - 1e-6 {
                            fill_span(pixels, surface_w, split, px + 1, py, cx0, cy0, cx1, cy1, c);
                            break;
                        }
                        if coverage > 0.0 {
                            put_pixel_aa(
                                pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage,
                            );
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
