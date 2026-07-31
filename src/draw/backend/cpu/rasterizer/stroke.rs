//! 帧执行器复用的矩形描边纯函数。

use crate::core::Rect;

use super::{
    align_rounded_rect, clip_to_int, color_to_premul, fill_rect_raw, intersect_rect, put_pixel_aa,
    rounded_rect_sdf, sdf_to_coverage,
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
    // 圆角矩形对齐物理像素网格：亚像素坐标下 SDF 弧线端点与像素中心错位，
    // 导致四角取整不对称（顶/底圆角视觉半径不一致）。直角矩形无此问题。
    let rect = if rad.tl != 0.0 || rad.tr != 0.0 || rad.bl != 0.0 || rad.br != 0.0 {
        match align_rounded_rect(rect) {
            Some(rect) => rect,
            None => return,
        }
    } else {
        rect
    };
    // 圆角描边使用"外扩 outer + 内缩 inner"双 SDF(与 GPU 后端一致):
    // 外扩 half 使弧线端点对齐像素中心,消除整数坐标下顶/底圆角起点偏差。
    let outer_rect = Rect::new(rect.x - h, rect.y - h, rect.w + lw, rect.h + lw);
    let inner_rect = Rect::new(
        rect.x + h,
        rect.y + h,
        (rect.w - lw).max(0.0),
        (rect.h - lw).max(0.0),
    );
    let outer_rad = Radius {
        tl: rad.tl + h,
        tr: rad.tr + h,
        br: rad.br + h,
        bl: rad.bl + h,
    };
    let inner_rad = Radius {
        tl: (rad.tl - h).max(0.0),
        tr: (rad.tr - h).max(0.0),
        br: (rad.br - h).max(0.0),
        bl: (rad.bl - h).max(0.0),
    };
    let expand = h + 1.0;
    let expanded = Rect::new(
        outer_rect.x - expand,
        outer_rect.y - expand,
        outer_rect.w + expand * 2.0,
        outer_rect.h + expand * 2.0,
    );
    if let Some(cr) = intersect_rect(&expanded, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
        let inner_is_positive = inner_rect.w > 0.0 && inner_rect.h > 0.0;
        for py in y0..y1 {
            let uy = py as f32 + 0.5;
            for px in x0..x1 {
                let ux = px as f32 + 0.5;
                let outer_sd = rounded_rect_sdf(ux, uy, &outer_rect, &outer_rad);
                let coverage = if inner_is_positive {
                    let inner_sd = rounded_rect_sdf(ux, uy, &inner_rect, &inner_rad);
                    sdf_to_coverage(outer_sd) * sdf_to_coverage(-inner_sd)
                } else {
                    // 描边宽度盖满矩形:直接填充外扩圆角矩形。
                    sdf_to_coverage(outer_sd)
                };
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
    fn subpixel_rounded_stroke_corners_are_symmetric() {
        // 与填充同构：亚像素 frame 下圆角描边四角取整不对称，
        // 对齐物理像素网格 + 双 SDF 后必须旋转对称。
        let (w, h) = (64i32, 64i32);
        let mut pixels = vec![0u32; (w * h) as usize];
        let rect = Rect::new(8.75, 5.0, 35.0, 35.0);
        // 与实现相同的边界对齐：左右/上下边界分别就近取整。
        let aligned = align_rounded_rect(rect).unwrap();
        let clip = Rect::new(0.0, 0.0, w as f32, h as f32);
        stroke_rect(
            &mut pixels,
            w,
            h,
            clip,
            1.0,
            rect,
            Color::WHITE,
            1.0,
            Some(Radius::uniform(7.5)),
        );
        let diff = rotation_symmetry_diff(&pixels, w, h, aligned);
        assert!(
            diff < 0.5,
            "圆角描边四角不对称：绕对齐矩形中心旋转 180° 的 alpha 差 = {diff}"
        );
    }

    #[test]
    fn subpixel_rounded_stroke_keeps_line_width() {
        let (w, h) = (64i32, 64i32);
        let mut pixels = vec![0u32; (w * h) as usize];
        let rect = Rect::new(8.75, 5.0, 35.0, 35.0);
        // 与实现相同的边界对齐：左右/上下边界分别就近取整。
        let aligned = align_rounded_rect(rect).unwrap();
        let clip = Rect::new(0.0, 0.0, w as f32, h as f32);
        let lw = 1.0;
        stroke_rect(
            &mut pixels,
            w,
            h,
            clip,
            1.0,
            rect,
            Color::WHITE,
            lw,
            Some(Radius::uniform(7.5)),
        );
        // 顶边中心（远离圆角）描边宽度应保持 lw：alpha>0 的行跨度为 lw 像素。
        let mid_x = (aligned.x + aligned.w * 0.5).floor() as i32;
        let y0 = (aligned.y - 2.0).max(0.0) as i32;
        // 只检查顶边附近几行，避免把底边计入。
        let y1 = (aligned.y + 3.0).min(h as f32) as i32;
        let mut top_rows = 0;
        for py in y0..y1 {
            let a = pixels[(py * w + mid_x) as usize] >> 24;
            if a > 0 {
                top_rows += 1;
            }
        }
        assert!(
            top_rows >= 1 && top_rows <= 2,
            "1px 圆角描边顶边实际厚度异常：{top_rows} 行"
        );
    }
}
