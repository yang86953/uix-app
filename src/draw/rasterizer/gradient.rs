//! 渐变填充纯函数——线性渐变、径向渐变。

use crate::native::Rect;

use crate::draw::primitives::color::Color;
use crate::draw::primitives::types::GradientDirection;

use super::{clip_to_int, color_to_premul, intersect_rect, put_pixel_aa};

/// 纯函数：线性渐变填充。
pub fn fill_linear_gradient(
    pixels: &mut [u32],
    surface_w: i32,
    _surface_h: i32,
    clip: Rect,
    opacity: f32,
    rect: Rect,
    color_a: Color,
    color_b: Color,
    dir: GradientDirection,
) {
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    if let Some(cr) = intersect_rect(&rect, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let ux = px as f32 + 0.5;
                let uy = py as f32 + 0.5;
                let local_x = ux - rect.x;
                let local_y = uy - rect.y;
                let t = match dir {
                    GradientDirection::Horizontal => local_x / rect.w.max(1.0),
                    GradientDirection::Vertical => local_y / rect.h.max(1.0),
                    GradientDirection::DiagonalTLBR => {
                        (local_x + local_y) / (rect.w + rect.h).max(1.0)
                    }
                    GradientDirection::DiagonalBLTR => {
                        (local_x - local_y + rect.h) / (rect.w + rect.h).max(1.0)
                    }
                };
                let t = t.clamp(0.0, 1.0);
                let r = (color_a.r as f32 * (1.0 - t) + color_b.r as f32 * t) as u8;
                let g = (color_a.g as f32 * (1.0 - t) + color_b.g as f32 * t) as u8;
                let b = (color_a.b as f32 * (1.0 - t) + color_b.b as f32 * t) as u8;
                let a = (color_a.a as f32 * (1.0 - t) + color_b.a as f32 * t) as u8;
                let blended = color_to_premul(r, g, b, a, opacity);
                put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, blended, 1.0);
            }
        }
    }
}

/// 纯函数：径向渐变填充。
pub fn fill_radial_gradient(
    pixels: &mut [u32],
    surface_w: i32,
    _surface_h: i32,
    clip: Rect,
    opacity: f32,
    cx: f32,
    cy: f32,
    inner_r: f32,
    outer_r: f32,
    inner_color: Color,
    outer_color: Color,
) {
    if outer_r <= 0.0 {
        return;
    }
    let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);
    let x0 = ((cx - outer_r).max(clip.x)) as i32;
    let y0 = ((cy - outer_r).max(clip.y)) as i32;
    let x1 = ((cx + outer_r).min(clip.x + clip.w)) as i32;
    let y1 = ((cy + outer_r).min(clip.y + clip.h)) as i32;
    let range = outer_r - inner_r;
    for py in y0..y1 {
        for px in x0..x1 {
            let dist = ((px as f32 + 0.5 - cx).powi(2) + (py as f32 + 0.5 - cy).powi(2)).sqrt();
            if dist > outer_r {
                continue;
            }
            let t = ((dist - inner_r) / range).clamp(0.0, 1.0);
            let r = (inner_color.r as f32 * (1.0 - t) + outer_color.r as f32 * t) as u8;
            let g = (inner_color.g as f32 * (1.0 - t) + outer_color.g as f32 * t) as u8;
            let b = (inner_color.b as f32 * (1.0 - t) + outer_color.b as f32 * t) as u8;
            let a = (inner_color.a as f32 * (1.0 - t) + outer_color.a as f32 * t) as u8;
            let blended = color_to_premul(r, g, b, a, opacity);
            put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, blended, 1.0);
        }
    }
}
