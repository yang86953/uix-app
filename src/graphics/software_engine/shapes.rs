use super::core::RenderTarget;
use crate::graphics::{Color, Radius, Rect};

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

        // 统一 SDF 填充
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

    /// 描边实现：
    ///   尖角→4 边 1D 距离场取 MAX（硬拼接）；
    ///   圆角→统一 SDF = |rounded_rect_sdf| - h（与 fill_rect 共用同一距离场，
    ///     直边与弧段梯度连续、覆盖完全一致，消除扇区边界不对称）。
    fn _stroke_rect_impl(&mut self, rect: Rect, color: Color, lw: f32, rad: Radius) {
        let c = self.apply_opacity(Self::premul(color));

        if rad.tl == 0.0
            && rad.tr == 0.0
            && rad.bl == 0.0
            && rad.br == 0.0
            && self.supersample_level() <= 1
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
                // Very thick stroke becomes a filled rect.
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

        // ── 矩形描边：统一 SDF = |rounded_rect_sdf| - h ──
        let h = lw * 0.5;
        let expand = h + 1.0;
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
                                let stroke_sd = sd.abs() - h;
                                sum += Self::sdf_to_coverage(stroke_sd);
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
                        if let Some((ux, uy)) = self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5) {
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

        // 梯度归一化椭圆 SDF：sd = (v-1)/|∇v|，与 sdf_to_coverage 配合
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
}
