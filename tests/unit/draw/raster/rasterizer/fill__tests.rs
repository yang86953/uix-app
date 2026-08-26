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
        // 正矩形必须可对齐；附带矩形几何便于定位亚像素输入。
        panic!("align_rounded_rect must succeed for a positive rect ({rect:?})");
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
