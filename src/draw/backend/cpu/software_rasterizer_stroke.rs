//! SoftwareRasterizer 矢量描边实现。

use crate::core::Rect;

use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::types::Radius;

use super::software_rasterizer::SoftwareRasterizer;

impl SoftwareRasterizer {
    #[allow(
        clippy::too_many_arguments,
        reason = "shape coordinates and surface bounds mirror the canvas contract"
    )]
    pub fn stroke_rect(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        rect: Rect,
        color: Color,
        line_width: f32,
        radius: Option<Radius>,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let rect = if ox != 0.0 || oy != 0.0 {
            Rect::new(rect.x + ox, rect.y + oy, rect.w, rect.h)
        } else {
            rect
        };
        let lw = line_width.max(0.0);
        let rad = radius.unwrap_or_default();
        let c = self.apply_opa(Self::premul(color));
        // Fast path for 1px axis-aligned
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
                self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0, w, h, c);
                return;
            }
            let inner_h = h - iw * 2;
            self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0, w, iw, c);
            self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0 + h - iw, w, iw, c);
            self.fill_rect_raw(pixels, surface_w, surface_h, x0, y0 + iw, iw, inner_h, c);
            self.fill_rect_raw(
                pixels,
                surface_w,
                surface_h,
                x0 + w - iw,
                y0 + iw,
                iw,
                inner_h,
                c,
            );
            return;
        }
        let h = lw * 0.5;
        // 圆角矩形对齐物理像素网格：亚像素坐标下 SDF 弧线端点与像素中心错位，
        // 导致四角取整不对称（顶/底圆角视觉半径不一致）。直角矩形无此问题。
        let rect = if rad.tl != 0.0 || rad.tr != 0.0 || rad.bl != 0.0 || rad.br != 0.0 {
            match super::rasterizer::core::align_rounded_rect(rect) {
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
        if let Some(cr) = self.intersect_clip(&expanded) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            let inner_is_positive = inner_rect.w > 0.0 && inner_rect.h > 0.0;
            for py in y0..y1 {
                let uy = py as f32 + 0.5;
                for px in x0..x1 {
                    let ux = px as f32 + 0.5;
                    let outer_sd = Self::rounded_rect_sdf(ux, uy, &outer_rect, &outer_rad);
                    let coverage = if inner_is_positive {
                        let inner_sd = Self::rounded_rect_sdf(ux, uy, &inner_rect, &inner_rad);
                        Self::sdf_to_coverage(outer_sd) * Self::sdf_to_coverage(-inner_sd)
                    } else {
                        // 描边宽度盖满矩形:直接填充外扩圆角矩形。
                        Self::sdf_to_coverage(outer_sd)
                    };
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
    pub fn stroke_circle(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        cx: f32,
        cy: f32,
        r: f32,
        color: Color,
        line_width: f32,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let (cx, cy) = if ox != 0.0 || oy != 0.0 {
            (cx + ox, cy + oy)
        } else {
            (cx, cy)
        };
        let lw = line_width.max(0.0);
        let c = self.apply_opa(Self::premul(color));
        let expand = r + lw * 0.5 + 1.0;
        let x0 = (cx - expand).max(self.clip_rect.x) as i32;
        let y0 = (cy - expand).max(self.clip_rect.y) as i32;
        let x1 = (cx + expand).min(self.clip_rect.x + self.clip_rect.w) as i32;
        let y1 = (cy + expand).min(self.clip_rect.y + self.clip_rect.h) as i32;
        for py in y0..y1 {
            for px in x0..x1 {
                let dx = px as f32 + 0.5 - cx;
                let dy = py as f32 + 0.5 - cy;
                let sd = (dx * dx + dy * dy).sqrt() - r;
                let coverage = Self::sdf_to_coverage(sd.abs() - lw * 0.5);
                if coverage > 0.0 {
                    self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                }
            }
        }
    }

    pub fn stroke_path(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        path: &Path,
        color: Color,
        opts: &StrokeOptions,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let path = if ox != 0.0 || oy != 0.0 {
            &path.translated(ox, oy)
        } else {
            path
        };
        let c = self.apply_opa(Self::premul(color));
        let stroked = crate::draw::geometry::stroker::stroke_path(path, opts);
        if stroked.is_empty() {
            return;
        }
        let polys = crate::draw::geometry::flattener::flatten(stroked.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        crate::draw::backend::cpu::rasterizer::polygon::fill_polygons(
            &polys,
            pixels,
            surface_w,
            surface_h,
            clip,
            c,
            FillRule::NonZero,
            &mut global_edges,
            &mut active_edges,
        );
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "line endpoints and surface bounds mirror the canvas contract"
    )]
    pub fn draw_line(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        mut x1: f32,
        mut y1: f32,
        mut x2: f32,
        mut y2: f32,
        color: Color,
        width: f32,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        x1 += ox;
        y1 += oy;
        x2 += ox;
        y2 += oy;
        let c = self.apply_opa(Self::premul(color));
        let half_lw = width.max(0.0) * 0.5;

        if (x1 - x2).abs() < 1e-6 {
            let x = x1 - half_lw;
            let y = y1.min(y2);
            self.fill_rect_raw(
                pixels,
                surface_w,
                surface_h,
                x as i32,
                y as i32,
                width as i32,
                (y1 - y2).abs() as i32,
                c,
            );
            return;
        }
        if (y1 - y2).abs() < 1e-6 {
            let x = x1.min(x2);
            let y = y1 - half_lw;
            self.fill_rect_raw(
                pixels,
                surface_w,
                surface_h,
                x as i32,
                y as i32,
                (x1 - x2).abs() as i32,
                width as i32,
                c,
            );
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
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }
}
