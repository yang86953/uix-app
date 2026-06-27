//! CpuCanvas2D 内部辅助方法（不在 Canvas2D trait 中）。

use uix_platform::Rect;

use crate::color::Color;
use crate::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::types::Radius;

impl CpuCanvas2D {
    pub(crate) fn _draw_box_shadow_impl(
        &mut self, rect: Rect, blur_radius: f32,
        offset_x: f32, offset_y: f32,
        shadow_color: Color, corner_radius: Option<Radius>, ambient: bool,
    ) {
        let rad = corner_radius.unwrap_or_default();
        let blur = blur_radius.max(0.0);
        if shadow_color.a == 0 { return; }
        let color = self.apply_opacity(Self::premul(shadow_color));
        let shadow_rect = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);
        let expand = blur + 1.0;
        let bounds = Rect::new(
            shadow_rect.x - expand, shadow_rect.y - expand,
            shadow_rect.w + expand * 2.0, shadow_rect.h + expand * 2.0,
        );
        let use_blur = blur > 0.5;
        if let Some(cr) = self.intersect_clip(&bounds) {
            let x0 = cr.x as i32; let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32; let y1 = (cr.y + cr.h) as i32;
            if x0 >= x1 || y0 >= y1 { return; }
            for py in y0..y1 { for px in x0..x1 {
                let ux = px as f32 + 0.5; let uy = py as f32 + 0.5;
                let sd = Self::rounded_rect_sdf(ux, uy, &shadow_rect, &rad);
                let coverage = if use_blur {
                    if ambient { Self::shadow_coverage_ambient(sd, blur) } else { Self::shadow_coverage(sd, blur) }
                } else { Self::sdf_to_coverage(sd) };
                if coverage > 0.0 { self.put_pixel_aa(px, py, color, coverage); }
            }}
        }
    }

    pub(crate) fn _stroke_rect_impl(&mut self, rect: Rect, color: Color, lw: f32, rad: Radius) {
        let c = self.apply_opacity(Self::premul(color));
        if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0
            && rect.x.fract() == 0.0 && rect.y.fract() == 0.0
            && rect.w.fract() == 0.0 && rect.h.fract() == 0.0
            && lw == 1.0 && lw.fract() == 0.0
        {
            let x0 = rect.x as i32; let y0 = rect.y as i32;
            let w = rect.w as i32; let h = rect.h as i32;
            let iw = lw as i32;
            if iw * 2 >= w || iw * 2 >= h { self.fill_rect_raw(x0, y0, w, h, c); return; }
            let inner_h = h - iw * 2;
            self.fill_rect_raw(x0, y0, w, iw, c);
            self.fill_rect_raw(x0, y0 + h - iw, w, iw, c);
            self.fill_rect_raw(x0, y0 + iw, iw, inner_h, c);
            self.fill_rect_raw(x0 + w - iw, y0 + iw, iw, inner_h, c);
            return;
        }
        let h = lw * 0.5;
        let expand = h + 1.0;
        let expanded = Rect::new(rect.x - expand, rect.y - expand,
            rect.w + expand * 2.0, rect.h + expand * 2.0);
        if let Some(cr) = self.intersect_clip(&expanded) {
            let x0 = cr.x as i32; let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32; let y1 = (cr.y + cr.h) as i32;
            for py in y0..y1 { for px in x0..x1 {
                let ux = px as f32 + 0.5; let uy = py as f32 + 0.5;
                let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                let coverage = Self::sdf_to_coverage(sd.abs() - h);
                if coverage > 0.0 { self.put_pixel_aa(px, py, c, coverage); }
            }}
        }
    }
}
