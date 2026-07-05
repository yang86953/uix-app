//! 阴影绘制纯函数——盒阴影（定向+环境）。

use crate::platform::Rect;

use crate::render::color::Color;
use crate::render::types::Radius;

use super::{
    clip_to_int, color_to_premul, intersect_rect, put_pixel_aa, rounded_rect_sdf, sdf_to_coverage,
    shadow_coverage, shadow_coverage_ambient,
};

/// 纯函数：绘制盒阴影。
pub fn draw_box_shadow(
    pixels: &mut [u32],
    surface_w: i32,
    surface_h: i32,
    clip: Rect,
    opacity: f32,
    rect: Rect,
    blur: f32,
    ox: f32,
    oy: f32,
    color: Color,
    corner_radius: Option<Radius>,
) {
    _draw_box_shadow_impl(
        pixels,
        surface_w,
        surface_h,
        clip,
        opacity,
        rect,
        blur,
        ox,
        oy,
        color,
        corner_radius,
        false,
    )
}

/// 纯函数：绘制环境阴影（更柔和的弥散）。
pub fn draw_box_shadow_ambient(
    pixels: &mut [u32],
    surface_w: i32,
    surface_h: i32,
    clip: Rect,
    opacity: f32,
    rect: Rect,
    blur: f32,
    ox: f32,
    oy: f32,
    color: Color,
    corner_radius: Option<Radius>,
) {
    _draw_box_shadow_impl(
        pixels,
        surface_w,
        surface_h,
        clip,
        opacity,
        rect,
        blur,
        ox,
        oy,
        color,
        corner_radius,
        true,
    )
}

fn _draw_box_shadow_impl(
    pixels: &mut [u32],
    surface_w: i32,
    _surface_h: i32,
    clip: Rect,
    opacity: f32,
    rect: Rect,
    blur_radius: f32,
    offset_x: f32,
    offset_y: f32,
    shadow_color: Color,
    corner_radius: Option<Radius>,
    ambient: bool,
) {
    let rad = corner_radius.unwrap_or_default();
    let blur = blur_radius.max(0.0);

    if shadow_color.a == 0 {
        return;
    }

    let c = color_to_premul(
        shadow_color.r,
        shadow_color.g,
        shadow_color.b,
        shadow_color.a,
        opacity,
    );
    let shadow_rect = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);

    let expand = blur + 1.0;
    let bounds = Rect::new(
        shadow_rect.x - expand,
        shadow_rect.y - expand,
        shadow_rect.w + expand * 2.0,
        shadow_rect.h + expand * 2.0,
    );

    let use_blur = blur > 0.5;

    if let Some(cr) = intersect_rect(&bounds, &clip) {
        let x0 = cr.x as i32;
        let y0 = cr.y as i32;
        let x1 = (cr.x + cr.w) as i32;
        let y1 = (cr.y + cr.h) as i32;
        let (cx0, cy0, cx1, cy1) = clip_to_int(&clip);

        if x0 >= x1 || y0 >= y1 {
            return;
        }

        for py in y0..y1 {
            for px in x0..x1 {
                let ux = px as f32 + 0.5;
                let uy = py as f32 + 0.5;
                let sd = rounded_rect_sdf(ux, uy, &shadow_rect, &rad);
                let coverage = if use_blur {
                    if ambient {
                        shadow_coverage_ambient(sd, blur)
                    } else {
                        shadow_coverage(sd, blur)
                    }
                } else {
                    sdf_to_coverage(sd)
                };
                if coverage > 0.0 {
                    put_pixel_aa(pixels, surface_w, px, py, cx0, cy0, cx1, cy1, c, coverage);
                }
            }
        }
    }
}
