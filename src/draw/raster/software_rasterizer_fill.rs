//! SoftwareRasterizer 矢量填充实现。

use crate::core::Rect;

use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::types::Radius;

use super::software_rasterizer::SoftwareRasterizer;

impl SoftwareRasterizer {
    pub(crate) fn fill_rect(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        rect: Rect,
        color: Color,
        radius: Option<Radius>,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        let c = self.apply_opa(Self::premul(color));
        let has_transform = !Self::is_identity(&self.transform);
        // 半径规范化每 draw call 一次，与 GPU shape/path lowering 同一 CSS 式规则。
        let radius =
            radius.map(|rad| super::rasterizer::core::normalize_corner_radius(rect.w, rect.h, rad));

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
                                self.put_pixel_aa(
                                    pixels, surface_w, surface_h, px, py, c, coverage,
                                );
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
                // 圆角矩形对齐物理像素网格：亚像素坐标下 SDF 弧线端点与像素中心
                // 错位，导致四角取整不对称（顶/底圆角视觉半径不一致）。
                let Some(rect) = super::rasterizer::core::align_rounded_rect(rect) else {
                    return;
                };
                let expanded = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
                if let Some(cr) = self.intersect_clip(&expanded) {
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
                                let coverage = Self::sdf_to_coverage(Self::rounded_rect_sdf(
                                    ux, uy, &rect, &rad,
                                ));
                                if coverage > 0.0 {
                                    self.put_pixel_aa(
                                        pixels, surface_w, surface_h, px, py, c, coverage,
                                    );
                                }
                            }
                            continue;
                        }

                        let uy = py as f32 + 0.5;
                        let mut px = x0;
                        while px < split {
                            let ux = px as f32 + 0.5;
                            let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                            let coverage = Self::sdf_to_coverage(sd);
                            if coverage >= 1.0 - 1e-6 {
                                self.fill_span(pixels, surface_w, surface_h, px, py, split - px, c);
                                break;
                            }
                            if coverage > 0.0 {
                                self.put_pixel_aa(
                                    pixels, surface_w, surface_h, px, py, c, coverage,
                                );
                            }
                            px += 1;
                        }

                        let mut px = x1;
                        while px > split {
                            px -= 1;
                            let ux = px as f32 + 0.5;
                            let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                            let coverage = Self::sdf_to_coverage(sd);
                            if coverage >= 1.0 - 1e-6 {
                                self.fill_span(
                                    pixels,
                                    surface_w,
                                    surface_h,
                                    split,
                                    py,
                                    px + 1 - split,
                                    c,
                                );
                                break;
                            }
                            if coverage > 0.0 {
                                self.put_pixel_aa(
                                    pixels, surface_w, surface_h, px, py, c, coverage,
                                );
                            }
                        }
                    }
                }
                return;
            }
        }

        let rad = radius.unwrap_or_default();
        let is_sharp = rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0;
        if is_sharp {
            let x0 = (rect.x + 0.5).floor() as i32;
            let y0 = (rect.y + 0.5).floor() as i32;
            let x1 = ((rect.x + rect.w) + 0.5).floor() as i32;
            let y1 = ((rect.y + rect.h) + 0.5).floor() as i32;
            let cw = x1 - x0;
            let ch = y1 - y0;
            if cw > 0 && ch > 0 {
                self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0, cw, ch, c);
            }
            return;
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
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "shape coordinates and surface bounds mirror the canvas contract"
    )]
    pub(crate) fn fill_circle(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let (cx, cy) = if ox != 0.0 || oy != 0.0 {
            (cx + ox, cy + oy)
        } else {
            (cx, cy)
        };
        let c = self.apply_opa(Self::premul(color));
        let expand = r + 1.0;
        let inner2 = (r - 1.0).max(0.0).powi(2);
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
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, 1.0);
                        continue;
                    }
                    if dist2 >= outer2 {
                        continue;
                    }
                    let coverage = Self::sdf_to_coverage(dist2.sqrt() - r);
                    if coverage > 0.0 {
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
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
                            let dist = ((ux - cx).powi(2) + (uy - cy).powi(2)).sqrt();
                            let coverage = Self::sdf_to_coverage(dist - r);
                            if coverage > 0.0 {
                                self.put_pixel_aa(
                                    pixels, surface_w, surface_h, px, py, c, coverage,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn fill_ellipse(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        rect: Rect,
        color: Color,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        let c = self.apply_opa(Self::premul(color));
        let (cx, cy) = (rect.x + rect.w / 2.0, rect.y + rect.h / 2.0);
        let (rx, ry) = (rect.w / 2.0, rect.h / 2.0);
        if rx <= 0.0 || ry <= 0.0 {
            return;
        }
        let inv_rx2 = 1.0 / (rx * rx);
        let inv_ry2 = 1.0 / (ry * ry);
        let expanded = Rect::new(rect.x - 1.0, rect.y - 1.0, rect.w + 2.0, rect.h + 2.0);
        // 非 identity transform 下先把保守局部边界映射到 surface。
        let device_bounds = if Self::is_identity(&self.transform) {
            // identity 继续沿用原始椭圆快速路径边界。
            expanded
        } else {
            // 任意可逆仿射由 inverse sampling 恢复局部椭圆坐标。
            self.transform_rect(&expanded)
        };
        // 只扫描变换后边界与当前 surface clip 的交集。
        if let Some(cr) = self.intersect_clip(&device_bounds) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    // identity 直接使用 surface 像素中心；仿射路径先逆映射到局部。
                    let Some((ux, uy)) = (if Self::is_identity(&self.transform) {
                        // 无变换时避免每像素读取逆矩阵。
                        Some((px as f32 + 0.5, py as f32 + 0.5))
                    } else {
                        // 退化或不可逆矩阵不产生像素。
                        self.apply_inverse(px as f32 + 0.5, py as f32 + 0.5)
                    }) else {
                        // 当前像素无法映射到局部空间。
                        continue;
                    };
                    // 计算局部椭圆归一化坐标。
                    let dx = ux - cx;
                    // 计算局部垂直距离。
                    let dy = uy - cy;
                    let v = dx * dx * inv_rx2 + dy * dy * inv_ry2;
                    if v >= 1.15 {
                        continue;
                    }
                    if v <= 0.85 {
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, 1.0);
                        continue;
                    }
                    let grad_mag =
                        2.0 * (dx * dx * inv_rx2 * inv_rx2 + dy * dy * inv_ry2 * inv_ry2).sqrt();
                    let coverage = Self::sdf_to_coverage((v - 1.0) / grad_mag.max(1e-12));
                    if coverage > 0.0 {
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "shape coordinates and surface bounds mirror the canvas contract"
    )]
    pub(crate) fn fill_sector(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        cx: f32,
        cy: f32,
        r: f32,
        start_angle: f32,
        end_angle: f32,
        color: Color,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let (cx, cy) = if ox != 0.0 || oy != 0.0 {
            (cx + ox, cy + oy)
        } else {
            (cx, cy)
        };
        let c = self.apply_opa(Self::premul(color));
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
                    if !in_sector(dy.atan2(dx)) {
                        continue;
                    }
                    let coverage = Self::sdf_to_coverage(dist2.sqrt() - r);
                    if coverage > 0.0 {
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
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
                            let dist = ((ux - cx).powi(2) + (uy - cy).powi(2)).sqrt();
                            if dist >= r {
                                continue;
                            }
                            if !in_sector((uy - cy).atan2(ux - cx)) {
                                continue;
                            }
                            let coverage = Self::sdf_to_coverage(dist - r);
                            if coverage > 0.0 {
                                self.put_pixel_aa(
                                    pixels, surface_w, surface_h, px, py, c, coverage,
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    pub(crate) fn fill_path(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        path: &Path,
        color: Color,
        fill_rule: FillRule,
    ) {
        // 读取 transform 前先应用的本地像素 offset。
        let (ox, oy) = (self.offset_x, self.offset_y);
        // 仅在状态确实改变几何时分配变换后的路径。
        let transformed_path;
        // 软件 polygon rasterizer 消费 surface-space 路径，因此在 flatten 前完成映射。
        let path = if ox != 0.0 || oy != 0.0 || !Self::is_identity(&self.transform) {
            // 严格保持 offset 后 transform 的 Canvas2D 顺序。
            transformed_path = path.translated(ox, oy).transformed(self.transform);
            // 借用本次调用内有效的 surface-space 路径。
            &transformed_path
        } else {
            // identity 且无 offset 时避免复制路径。
            path
        };
        let c = self.apply_opa(Self::premul(color));
        let polys = crate::draw::geometry::flattener::flatten(path.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        crate::draw::raster::rasterizer::polygon::fill_polygons(
            &polys,
            pixels,
            surface_w,
            surface_h,
            clip,
            c,
            fill_rule,
            &mut global_edges,
            &mut active_edges,
        );
    }
}
