use super::core::RenderTarget;
use crate::base::{Point, Rect};
use crate::graphics::path::FillRule;
use crate::graphics::rasterizer;
use crate::graphics::Color;

impl RenderTarget {
    /// Fill a circle with radial alpha gradient: full color.a at center → 0 at edge.
    pub fn fill_circle_radial(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let c = self.apply_opacity(Self::premul(color));
        let expand = r + 1.0;
        let outer2 = (r + 1.0) * (r + 1.0);
        if Self::is_identity(&self.transform) {
            let x0 = (cx - expand).max(self.clip_rect.x) as i32;
            let y0 = (cy - expand).max(self.clip_rect.y) as i32;
            let x1 = (cx + expand).min(self.clip_rect.x + self.clip_rect.w) as i32;
            let y1 = (cy + expand).min(self.clip_rect.y + self.clip_rect.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;
                    let dist2 = dx * dx + dy * dy;
                    if dist2 >= outer2 {
                        continue;
                    }
                    let dist = dist2.sqrt();
                    let radial = 1.0 - (dist / r).min(1.0);
                    let coverage = Self::sdf_to_coverage(dist - r);
                    let effective = (radial * coverage).min(1.0);
                    if effective > 0.0 {
                        self.put_pixel_aa(px, py, c, effective);
                    }
                }
            }
        } else {
            let bb = Rect::new(cx - expand, cy - expand, expand * 2.0, expand * 2.0);
            let bounds = self.transform_rect(&bb);
            if let Some(cr) = self.intersect_clip(&bounds) {
                for py in (cr.y as i32)..((cr.y + cr.h) as i32) {
                    for px in (cr.x as i32)..((cr.x + cr.w) as i32) {
                        if let Some((ux, uy)) = self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                        {
                            let dx = ux - cx;
                            let dy = uy - cy;
                            let dist = (dx * dx + dy * dy).sqrt();
                            let radial = 1.0 - (dist / r).min(1.0);
                            let coverage = Self::sdf_to_coverage(dist - r);
                            let effective = (radial * coverage).min(1.0);
                            if effective > 0.0 {
                                self.put_pixel_aa(px, py, c, effective);
                            }
                        }
                    }
                }
            }
        }
    }

    /// Fill a circular sector (pie slice). Angles in radians. 0 = 3 o'clock. CCW sweep.
    pub fn fill_sector(
        &mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        let c = self.apply_opacity(Self::premul(color));
        let expand = r + 1.0;
        let norm = |a: f32| a.rem_euclid(std::f32::consts::TAU);
        let sa = norm(start_angle);
        let ea = norm(end_angle);
        let in_sector = |angle: f32| -> bool {
            let a = norm(angle);
            if sa <= ea {
                a >= sa && a <= ea
            } else {
                a >= sa || a <= ea
            }
        };
        let outer2 = (r + 1.0) * (r + 1.0);
        if Self::is_identity(&self.transform) {
            let x0 = (cx - expand).max(self.clip_rect.x) as i32;
            let y0 = (cy - expand).max(self.clip_rect.y) as i32;
            let x1 = (cx + expand).min(self.clip_rect.x + self.clip_rect.w) as i32;
            let y1 = (cy + expand).min(self.clip_rect.y + self.clip_rect.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;
                    let dist2 = dx * dx + dy * dy;
                    if dist2 >= outer2 {
                        continue;
                    }
                    let angle = dy.atan2(dx);
                    if !in_sector(angle) {
                        continue;
                    }
                    let coverage = Self::sdf_to_coverage(dist2.sqrt() - r);
                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        } else {
            let bb = Rect::new(cx - expand, cy - expand, expand * 2.0, expand * 2.0);
            let bounds = self.transform_rect(&bb);
            if let Some(cr) = self.intersect_clip(&bounds) {
                for py in (cr.y as i32)..((cr.y + cr.h) as i32) {
                    for px in (cr.x as i32)..((cr.x + cr.w) as i32) {
                        if let Some((ux, uy)) = self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                        {
                            let dx = ux - cx;
                            let dy = uy - cy;
                            let dist = (dx * dx + dy * dy).sqrt();
                            if dist >= r {
                                continue;
                            }
                            let angle = dy.atan2(dx);
                            if !in_sector(angle) {
                                continue;
                            }
                            let coverage = Self::sdf_to_coverage(dist - r);
                            if coverage > 0.0 {
                                self.put_pixel_aa(px, py, c, coverage);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
        let c = self.apply_opacity(Self::premul(color));
        let expand = r + 1.0;
        let inner = (r - 1.0).max(0.0);
        let inner2 = inner * inner;
        let outer2 = (r + 1.0) * (r + 1.0);
        if Self::is_identity(&self.transform) {
            let x0 = (cx - expand).max(self.clip_rect.x) as i32;
            let y0 = (cy - expand).max(self.clip_rect.y) as i32;
            let x1 = (cx + expand).min(self.clip_rect.x + self.clip_rect.w) as i32;
            let y1 = (cy + expand).min(self.clip_rect.y + self.clip_rect.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;
                    let dist2 = dx * dx + dy * dy;
                    if dist2 <= inner2 {
                        self.put_pixel_aa(px, py, c, 1.0);
                        continue;
                    }
                    if dist2 >= outer2 {
                        continue;
                    }
                    let dist = dist2.sqrt();
                    let coverage = Self::sdf_to_coverage(dist - r);
                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        } else {
            let bb = Rect::new(cx - expand, cy - expand, expand * 2.0, expand * 2.0);
            let bounds = self.transform_rect(&bb);
            if let Some(cr) = self.intersect_clip(&bounds) {
                for py in (cr.y as i32)..((cr.y + cr.h) as i32) {
                    for px in (cr.x as i32)..((cr.x + cr.w) as i32) {
                        if let Some((ux, uy)) = self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                        {
                            let dx = ux - cx;
                            let dy = uy - cy;
                            let dist = (dx * dx + dy * dy).sqrt();
                            let coverage = Self::sdf_to_coverage(dist - r);
                            if coverage > 0.0 {
                                self.put_pixel_aa(px, py, c, coverage);
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
        let lw = line_width.max(0.0);
        let c = self.apply_opacity(Self::premul(color));
        let expand = r + lw * 0.5 + 1.0;
        let x0 = (cx - expand).max(self.clip_rect.x) as i32;
        let y0 = (cy - expand).max(self.clip_rect.y) as i32;
        let x1 = (cx + expand).min(self.clip_rect.x + self.clip_rect.w) as i32;
        let y1 = (cy + expand).min(self.clip_rect.y + self.clip_rect.h) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let dist = (dx * dx + dy * dy).sqrt();
                let sd = dist - r;
                let stroke_sd = sd.abs() - lw * 0.5;
                let coverage = Self::sdf_to_coverage(stroke_sd);
                if coverage > 0.0 {
                    self.put_pixel_aa(px, py, c, coverage);
                }
            }
        }
    }

    pub fn fill_ellipse(&mut self, rect: Rect, color: Color) {
        let c = self.apply_opacity(Self::premul(color));
        let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let (rx, ry) = (rect.w / 2.0, rect.h / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let inv_rx2 = 1.0 / (rx * rx);
        let inv_ry2 = 1.0 / (ry * ry);
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
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;
                    let tx = dx * dx * inv_rx2;
                    let ty = dy * dy * inv_ry2;
                    let v = tx + ty;
                    if v >= 1.15 {
                        continue;
                    }
                    if v <= 0.85 {
                        self.put_pixel_aa(px, py, c, 1.0);
                        continue;
                    }
                    let grad_mag = 2.0 * (tx * inv_rx2 + ty * inv_ry2).sqrt();
                    let sd = (v - 1.0) / grad_mag.max(1e-12);
                    let coverage = Self::sdf_to_coverage(sd);
                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        }
    }

    pub fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
        let c = self.apply_opacity(Self::premul(color));
        let half_lw = width.max(0.0) * 0.5;

        if (x1 - x2).abs() < 1e-6 {
            let x = x1 - half_lw;
            let y = y1.min(y2);
            let w = width;
            let h = (y1 - y2).abs();
            self.fill_rect(Rect::new(x, y, w, h), color, None);
            return;
        }
        if (y1 - y2).abs() < 1e-6 {
            let x = x1.min(x2);
            let y = y1 - half_lw;
            let w = (x1 - x2).abs();
            let h = width;
            self.fill_rect(Rect::new(x, y, w, h), color, None);
            return;
        }

        let expand = half_lw + 1.0;
        let bb = Rect::new(
            x1.min(x2) - expand,
            y1.min(y2) - expand,
            (x1 - x2).abs() + expand * 2.0,
            (y1 - y2).abs() + expand * 2.0,
        );
        if let Some(cr) = self.intersect_clip(&bb) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1b = (cr.x + cr.w) as i32;
            let y1b = (cr.y + cr.h) as i32;
            for py in y0..y1b {
                for px in x0..x1b {
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = Self::line_segment_sdf(ux, uy, x1, y1, x2, y2) - half_lw;
                    let coverage = Self::sdf_to_coverage(sd);
                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        }
    }

    /// 用扫描线栅格器填充展平后的多边形（带 clip 检查）。
    pub(crate) fn fill_polygons_with_opacity(
        &mut self,
        polys: &[Vec<Point>],
        clip: Rect,
        color: u32,
        fill_rule: FillRule,
    ) {
        let w = self.width();
        let h = self.height();
        if let Some(cr) = self.intersect_clip(&clip) {
            let mut global_edges = Vec::new();
            let mut active_edges = Vec::new();
            rasterizer::fill_polygons(
                polys,
                self.pixel_buffer_mut(),
                w,
                h,
                cr,
                color,
                fill_rule,
                &mut global_edges,
                &mut active_edges,
            );
        }
    }
}
