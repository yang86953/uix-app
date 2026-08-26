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
    let Some(aligned) = align_rounded_rect(rect) else {
        // 正矩形必须可对齐；附带矩形几何便于定位亚像素输入。
        panic!("align_rounded_rect must succeed for a positive rect ({rect:?})");
    };
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
    let Some(aligned) = align_rounded_rect(rect) else {
        // 正矩形必须可对齐；附带矩形几何便于定位亚像素输入。
        panic!("align_rounded_rect must succeed for a positive rect ({rect:?})");
    };
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
