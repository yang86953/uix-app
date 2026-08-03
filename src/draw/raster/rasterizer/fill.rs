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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::draw::geometry::color::Color;

    /// 渲染结果绕"对齐后矩形中心"旋转 180° 的逐像素 alpha 差。
    fn rotation_symmetry_diff(pixels: &[u32], surface_w: i32, surface_h: i32, rect: Rect) -> f64 {
        let cx = rect.x + rect.w * 0.5;
        let cy = rect.y + rect.h * 0.5;
        let x0 = rect.x.floor().max(0.0) as i32 - 1;
        let y0 = rect.y.floor().max(0.0) as i32 - 1;
        let x1 = (rect.x + rect.w).ceil().min(surface_w as f32) as i32 + 1;
        let y1 = (rect.y + rect.h).ceil().min(surface_h as f32) as i32 + 1;
        let mut diff = 0.0;
        for py in y0.max(0)..y1.min(surface_h) {
            for px in x0.max(0)..x1.min(surface_w) {
                let sx = (2.0 * cx - (px as f32 + 0.5) - 0.5).round() as i32;
                let sy = (2.0 * cy - (py as f32 + 0.5) - 0.5).round() as i32;
                if sx >= 0 && sx < surface_w && sy >= 0 && sy < surface_h {
                    let a = (pixels[(py * surface_w + px) as usize] >> 24) as f64;
                    let b = (pixels[(sy * surface_w + sx) as usize] >> 24) as f64;
                    diff += (a - b).abs();
                }
            }
        }
        diff
    }

    #[test]
    fn subpixel_rounded_fill_corners_are_symmetric() {
        // DPR 缩放（如 125%）下按钮 frame 落亚像素：SDF 弧线端点与像素中心
        // 错位会让四角取整不对称；对齐物理像素网格后四角必须旋转对称。
        let (w, h) = (64i32, 64i32);
        let mut pixels = vec![0u32; (w * h) as usize];
        let rect = Rect::new(8.75, 5.0, 35.0, 35.0);
        // 与实现相同的边界对齐：左右/上下边界分别就近取整。
        let Some(aligned) = align_rounded_rect(rect) else {
            panic!("align_rounded_rect must succeed for a positive rect");
        };
        let clip = Rect::new(0.0, 0.0, w as f32, h as f32);
        fill_rect(
            &mut pixels,
            w,
            h,
            clip,
            1.0,
            rect,
            Color::WHITE,
            Some(Radius::uniform(7.5)),
        );
        let diff = rotation_symmetry_diff(&pixels, w, h, aligned);
        assert!(
            diff < 0.5,
            "圆角填充四角不对称：绕对齐矩形中心旋转 180° 的 alpha 差 = {diff}"
        );
    }

    #[test]
    fn narrow_rounded_fill_has_no_split_column_hole() {
        // 回归：优化路径左右循环在 split 列分界，右循环若提前退出（所有
        // 采样列 coverage < 1），split 列必须仍被采样，否则窄圆角矩形
        // （圆角盖满整行时）中列整列丢失。
        let (w, h) = (16i32, 16i32);
        let mut pixels = vec![0u32; (w * h) as usize];
        let rect = Rect::new(4.0, 4.0, 2.0, 2.0);
        let clip = Rect::new(0.0, 0.0, w as f32, h as f32);
        fill_rect(
            &mut pixels,
            w,
            h,
            clip,
            1.0,
            rect,
            Color::WHITE,
            Some(Radius::uniform(1.0)),
        );
        // 2x2 圆角矩形：四角半径为 1（盖满），中心 2x2 像素都应有覆盖。
        let mut covered = 0;
        for py in 4..6 {
            for px in 4..6 {
                if (pixels[(py * w + px) as usize] >> 24) > 0 {
                    covered += 1;
                }
            }
        }
        assert_eq!(
            covered, 4,
            "2px 宽圆角矩形存在空洞：中心 2x2 仅 {covered}/4 像素被覆盖"
        );
    }
}
