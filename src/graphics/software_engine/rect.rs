use super::core::RenderTarget;
use crate::graphics::{Color, Radius};
use crate::base::{Point, Rect};

impl RenderTarget {
    pub fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
        let c = self.apply_opacity(Self::premul(color));
        let has_transform = !Self::is_identity(&self.transform);

        if has_transform {
            let bounds = self.transform_rect(&rect);
            if let Some(cr) = self.intersect_clip(&bounds) {
                let x0 = cr.x as i32;
                let y0 = cr.y as i32;
                let x1 = (cr.x + cr.w) as i32;
                let y1 = (cr.y + cr.h) as i32;
                for py in y0..y1 {
                    for px in x0..x1 {
                        if let Some((ux, uy)) = self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                        {
                            let coverage = match radius {
                                None => {
                                    if ux >= rect.x
                                        && ux <= rect.x + rect.w
                                        && uy >= rect.y
                                        && uy <= rect.y + rect.h
                                    {
                                        1.0
                                    } else {
                                        0.0
                                    }
                                }
                                Some(ref rad) => Self::sdf_to_coverage(Self::rounded_rect_sdf(
                                    ux, uy, &rect, rad,
                                )),
                            };
                            if coverage > 0.0 {
                                self.put_pixel_aa(px, py, c, coverage);
                            }
                        }
                    }
                }
            }
            return;
        }

        // No transform
        if let Some(rad) = radius {
            if rad.tl != 0.0 || rad.tr != 0.0 || rad.bl != 0.0 || rad.br != 0.0 {
                let expanded = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
                if let Some(cr) = self.intersect_clip(&expanded) {
                    let x0 = cr.x as i32;
                    let y0 = cr.y as i32;
                    let x1 = (cr.x + cr.w) as i32;
                    let y1 = (cr.y + cr.h) as i32;
                    for py in y0..y1 {
                        for px in x0..x1 {
                            let ux = px as f32 + 0.5;
                            let uy = py as f32 + 0.5;
                            let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                            let coverage = Self::sdf_to_coverage(sd);
                            if coverage > 0.0 {
                                self.put_pixel_aa(px, py, c, coverage);
                            }
                        }
                    }
                }
                return;
            }
        }

        // Unified SDF fill
        let rad = radius.unwrap_or_default();
        let is_sharp = rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0;

        if is_sharp {
            if let Some(cr) = self.intersect_clip(&rect) {
                if cr.x.fract() == 0.0
                    && cr.y.fract() == 0.0
                    && (cr.x + cr.w).fract() == 0.0
                    && (cr.y + cr.h).fract() == 0.0
                {
                    self.fill_rect_raw(cr.x as i32, cr.y as i32, cr.w as i32, cr.h as i32, c);
                    return;
                }
            }
        }

        let expand = 1.0;
        let expanded = Rect::new(
            rect.x - expand,
            rect.y - expand,
            rect.w + expand * 2.0,
            rect.h + expand * 2.0,
        );
        if let Some(cr) = self.intersect_clip(&expanded) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                    let coverage = Self::sdf_to_coverage(sd);
                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        }
    }

    pub fn stroke_rect(
        &mut self,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        let lw = line_width.max(0.0);
        self._stroke_rect_impl(rect, color, lw, radius.unwrap_or_default())
    }

    fn _stroke_rect_impl(&mut self, rect: Rect, color: Color, lw: f32, rad: Radius) {
        let c = self.apply_opacity(Self::premul(color));

        if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0
            && self.supersample_level() <= 1
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
                self.fill_rect_raw(x0, y0, w, h, c);
                return;
            }
            let inner_h = h - iw * 2;
            self.fill_rect_raw(x0, y0, w, iw, c);
            self.fill_rect_raw(x0, y0 + h - iw, w, iw, c);
            self.fill_rect_raw(x0, y0 + iw, iw, inner_h, c);
            self.fill_rect_raw(x0 + w - iw, y0 + iw, iw, inner_h, c);
            return;
        }

        // Stroke via unified SDF: sd_abs - half_width
        let h = lw * 0.5;
        let expand = h + 1.0;
        let expanded = Rect::new(rect.x - expand, rect.y - expand,
            rect.w + expand * 2.0, rect.h + expand * 2.0);
        if let Some(cr) = self.intersect_clip(&expanded) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let sup = self.supersample_level();
                    let coverage = if sup > 1 {
                        let n = sup as usize;
                        let inv_n = 1.0 / (n as f32);
                        let mut sum = 0.0;
                        for iy in 0..n {
                            for ix in 0..n {
                                let sx = px as f32 + (ix as f32 + 0.5) * inv_n;
                                let sy = py as f32 + (iy as f32 + 0.5) * inv_n;
                                let sd = Self::rounded_rect_sdf(sx, sy, &rect, &rad);
                                sum += Self::sdf_to_coverage(sd.abs() - h);
                            }
                        }
                        (sum / (n * n) as f32).clamp(0.0, 1.0)
                    } else {
                        let ux = px as f32 + 0.5;
                        let uy = py as f32 + 0.5;
                        let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                        Self::sdf_to_coverage(sd.abs() - h)
                    };
                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        }
    }
}
