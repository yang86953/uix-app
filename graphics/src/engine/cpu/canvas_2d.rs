//! CPU 2D 绘制上下文——实现 Canvas2D trait。
//!
//! 组合 PixelSurface + 渲染状态栈。
//! 绘制方法委托 rasterizer 纯函数（待迁入），
//! 像素级操作直接访问底层 surface。

use uix_core::Rect;

use crate::color::Color;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::path::{FillRule, Path};
use crate::stroker::StrokeOptions;
use crate::traits::Canvas2D;
use crate::types::{BlendMode, GradientDirection, Radius, Transform};

/// 渲染状态快照（用于 save/restore）。
#[derive(Clone)]
struct StateSnapshot {
    pub(crate) clip_rect: Rect,
    pub(crate) clip_int: (i32, i32, i32, i32),
    pub(crate) opacity: f32,
    pub(crate) transform: Transform,
    pub(crate) invert: Option<[f64; 6]>,
    pub(crate) blend_mode: BlendMode,
}

/// CPU 2D 绘制实现。
pub struct CpuCanvas2D {
    /// 底层像素表面。
    pub(crate) surface: PixelSurface,

    /// 当前裁剪矩形（浮点）。
    pub(crate) clip_rect: Rect,
    /// 预计算的整数裁剪边界。
    pub(crate) clip_int: (i32, i32, i32, i32),

    /// 裁剪矩形栈。
    pub(crate) clip_stack: Vec<Rect>,

    /// 全局透明度。
    pub(crate) opacity: f32,

    /// 当前 2D 仿射变换。
    pub(crate) transform: Transform,
    /// 逆变换（懒计算缓存）。
    pub(crate) invert: Option<[f64; 6]>,

    /// 混合模式。
    pub(crate) blend_mode: BlendMode,

    /// 状态快照栈。
    pub(crate) state_stack: Vec<StateSnapshot>,
}

impl CpuCanvas2D {
    pub fn new(surface: PixelSurface) -> Self {
        let w = surface.surface_size().w as i32;
        let h = surface.surface_size().h as i32;
        Self {
            surface,
            clip_rect: Rect::new(0.0, 0.0, w as f32, h as f32),
            clip_int: (0, 0, w, h),
            clip_stack: Vec::new(),
            opacity: 1.0,
            transform: Transform::identity(),
            invert: Self::compute_inverse(&Transform::identity()),
            blend_mode: BlendMode::default(),
            state_stack: Vec::new(),
        }
    }

    /// 获取底层表面的引用。
    pub fn surface(&self) -> &PixelSurface {
        &self.surface
    }

    /// 获取底层表面的可变引用。
    pub fn surface_mut(&mut self) -> &mut PixelSurface {
        &mut self.surface
    }

    // ── 变换工具 ──

    pub(crate) fn is_identity(t: &Transform) -> bool {
        t.m[0] == 1.0 && t.m[1] == 0.0 && t.m[2] == 0.0
            && t.m[3] == 0.0 && t.m[4] == 1.0 && t.m[5] == 0.0
    }

    pub(crate) fn compute_inverse(t: &Transform) -> Option<[f64; 6]> {
        let [a, b, tx, c, d, ty] = t.m.map(|v| v as f64);
        let det = a * d - b * c;
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        Some([
            inv * d,
            inv * (-b),
            inv * (b * ty - d * tx),
            inv * (-c),
            inv * a,
            inv * (c * tx - a * ty),
        ])
    }

    pub(crate) fn apply_transform(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, tx, c, d, ty] = self.transform.m;
        (a * x + b * y + tx, c * x + d * y + ty)
    }

    pub(crate) fn apply_inverse(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.invert.map(|[a, b, tx, c, d, ty]| {
            let xf = x as f64;
            let yf = y as f64;
            ((a * xf + b * yf + tx) as f32, (c * xf + d * yf + ty) as f32)
        })
    }

    pub(crate) fn sync_clip_int(&mut self) {
        self.clip_int = (
            (self.clip_rect.x + 0.5).floor() as i32,
            (self.clip_rect.y + 0.5).floor() as i32,
            (self.clip_rect.x + self.clip_rect.w + 0.5).floor() as i32,
            (self.clip_rect.y + self.clip_rect.h + 0.5).floor() as i32,
        );
    }

    pub(crate) fn transform_rect(&self, r: &Rect) -> Rect {
        let (x1, y1) = self.apply_transform(r.x, r.y);
        let (x2, y2) = self.apply_transform(r.x + r.w, r.y);
        let (x3, y3) = self.apply_transform(r.x, r.y + r.h);
        let (x4, y4) = self.apply_transform(r.x + r.w, r.y + r.h);
        let min_x = x1.min(x2).min(x3).min(x4);
        let min_y = y1.min(y2).min(y3).min(y4);
        let max_x = x1.max(x2).max(x3).max(x4);
        let max_y = y1.max(y2).max(y3).max(y4);
        Rect::new(min_x, min_y, max_x - min_x, max_y - min_y)
    }

    pub(crate) fn intersect_clip(&self, r: &Rect) -> Option<Rect> {
        let x = r.x.max(self.clip_rect.x);
        let y = r.y.max(self.clip_rect.y);
        let rgt = (r.x + r.w).min(self.clip_rect.x + self.clip_rect.w);
        let bot = (r.y + r.h).min(self.clip_rect.y + self.clip_rect.h);
        if x < rgt && y < bot {
            Some(Rect::new(x, y, rgt - x, bot - y))
        } else {
            None
        }
    }

    // ── 颜色工具 ──

    pub(crate) fn premul(c: Color) -> u32 {
        let a = c.a as u32;
        let r = (c.r as u32 * a / 255).min(255);
        let g = (c.g as u32 * a / 255).min(255);
        let b = (c.b as u32 * a / 255).min(255);
        (a << 24) | (r << 16) | (g << 8) | b
    }

    pub(crate) fn apply_opacity(&self, c: u32) -> u32 {
        let a = ((c >> 24) & 0xFF) as f32 * self.opacity;
        let r = ((c >> 16) & 0xFF) as f32 * self.opacity;
        let g = ((c >> 8) & 0xFF) as f32 * self.opacity;
        let b = (c & 0xFF) as f32 * self.opacity;
        ((a as u32).min(255) << 24)
            | ((r as u32).min(255) << 16)
            | ((g as u32).min(255) << 8)
            | (b as u32).min(255)
    }

    /// Static version: apply an explicit opacity factor to a premultiplied color.
    pub(crate) fn apply_opacity_with(opacity: f32, color: u32) -> u32 {
        let a = ((color >> 24) & 0xFF) as f32 * opacity;
        let r = ((color >> 16) & 0xFF) as f32 * opacity;
        let g = ((color >> 8) & 0xFF) as f32 * opacity;
        let b = (color & 0xFF) as f32 * opacity;
        ((a as u32).min(255) << 24)
            | ((r as u32).min(255) << 16)
            | ((g as u32).min(255) << 8)
            | (b as u32).min(255)
    }

    // ── 像素操作 ──

    pub(crate) fn put_pixel_raw(&mut self, x: i32, y: i32, color: u32) {
        let w = self.surface.width();
        let h = self.surface.height();
        let (cx0, cy0, cx1, cy1) = self.clip_int;
        if x < cx0.max(0) || y < cy0.max(0) || x >= cx1.min(w) || y >= cy1.min(h) {
            return;
        }
        let idx = (y * w + x) as usize;
        let pixels = self.surface.pixels_mut();
        if idx >= pixels.len() {
            return;
        }
        let src_a = (color >> 24) & 0xFF;
        if src_a == 0 {
            return;
        }
        let dst = pixels[idx];
        let dst_a = (dst >> 24) & 0xFF;
        if src_a == 0xFF && dst_a == 0 {
            pixels[idx] = color;
            return;
        }
        let src_b = color & 0xFF;
        let src_g = (color >> 8) & 0xFF;
        let src_r = (color >> 16) & 0xFF;
        let dst_b = dst & 0xFF;
        let dst_g = (dst >> 8) & 0xFF;
        let dst_r = (dst >> 16) & 0xFF;
        let out_a = src_a + dst_a - (src_a * dst_a / 255);
        let out_r = src_r + (dst_r * (255 - src_a) / 255);
        let out_g = src_g + (dst_g * (255 - src_a) / 255);
        let out_b = src_b + (dst_b * (255 - src_a) / 255);
        pixels[idx] = out_a << 24 | out_r << 16 | out_g << 8 | out_b;
    }

    pub(crate) fn put_pixel_aa(&mut self, x: i32, y: i32, premul_color: u32, coverage: f32) {
        let (cx0, cy0, cx1, cy1) = self.clip_int;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 {
            return;
        }

        if coverage >= 1.0 - 1e-6 {
            self.put_pixel_raw(x, y, premul_color);
            return;
        }
        if coverage <= 0.0 {
            return;
        }

        let src_a = ((premul_color >> 24) & 0xFF) as f32;
        if src_a <= 0.0 {
            return;
        }

        let src_r_p = ((premul_color >> 16) & 0xFF) as f32 * coverage;
        let src_g_p = ((premul_color >> 8) & 0xFF) as f32 * coverage;
        let src_b_p = (premul_color & 0xFF) as f32 * coverage;
        let src_a_s = src_a * coverage;

        let w = self.surface.width();
        let h = self.surface.height();
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        let pixels = self.surface.pixels_mut();
        let idx = (y * w + x) as usize;
        if idx >= pixels.len() {
            return;
        }

        let dst = pixels[idx];
        let dst_a = ((dst >> 24) & 0xFF) as f32;
        let dst_r_p = ((dst >> 16) & 0xFF) as f32;
        let dst_g_p = ((dst >> 8) & 0xFF) as f32;
        let dst_b_p = (dst & 0xFF) as f32;

        let inv = 1.0 - (src_a_s / 255.0);
        let out_a = src_a_s + dst_a * inv;
        let out_r_p = src_r_p + dst_r_p * inv;
        let out_g_p = src_g_p + dst_g_p * inv;
        let out_b_p = src_b_p + dst_b_p * inv;

        let out_a_u = out_a.round() as u32;
        let out_r_u = out_r_p.round() as u32;
        let out_g_u = out_g_p.round() as u32;
        let out_b_u = out_b_p.round() as u32;

        pixels[idx] = (out_a_u.min(255) << 24)
            | (out_r_u.min(255) << 16)
            | (out_g_u.min(255) << 8)
            | out_b_u.min(255);
    }

    pub(crate) fn fill_span(&mut self, x: i32, y: i32, w: i32, color: u32) {
        if (color >> 24) == 0xFF {
            let tw = self.surface.width();
            let th = self.surface.height();
            let (cx0, cy0, cx1, cy1) = self.clip_int;
            let x0 = x.max(cx0).max(0);
            let x1 = (x + w).min(cx1).min(tw);
            if y >= cy0.max(0) && y < cy1.min(th) && x0 < x1 {
                let pixels = self.surface.pixels_mut();
                let start = (y * tw + x0) as usize;
                pixels[start..start + (x1 - x0) as usize].fill(color);
            }
            return;
        }
        for dx in 0..w {
            self.put_pixel_raw(x + dx, y, color);
        }
    }

    pub(crate) fn fill_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        for dy in 0..h {
            self.fill_span(x, y + dy, w, color);
        }
    }

    // ── SDF 工具（from software_engine/sdf.rs）──

    /// Signed distance field for a rounded rectangle with per-corner radii.
    pub(crate) fn rounded_rect_sdf(ux: f32, uy: f32, r: &Rect, rad: &Radius) -> f32 {
        if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0 {
            let dx = (r.x - ux).max(ux - (r.x + r.w)).max(0.0);
            let dy = (r.y - uy).max(uy - (r.y + r.h)).max(0.0);
            let outside = (dx * dx + dy * dy).sqrt();
            let inside = (r.x - ux)
                .max(ux - (r.x + r.w))
                .max((r.y - uy).max(uy - (r.y + r.h)));
            return if inside < 0.0 { inside } else { outside };
        }

        let cx = r.x + r.w * 0.5;
        let cy = r.y + r.h * 0.5;
        let half_w = r.w * 0.5;
        let half_h = r.h * 0.5;
        let px = ux - cx;
        let py = uy - cy;

        let cr = if px < 0.0 {
            if py < 0.0 { rad.tl } else { rad.bl }
        } else {
            if py < 0.0 { rad.tr } else { rad.br }
        };

        let qx = px.abs() - half_w + cr;
        let qy = py.abs() - half_h + cr;
        let qx_clamped = qx.max(0.0);
        let qy_clamped = qy.max(0.0);
        let outside = (qx_clamped * qx_clamped + qy_clamped * qy_clamped).sqrt();
        let inside = qx.max(qy).min(0.0);
        outside + inside - cr
    }

    /// Signed distance from point to a line segment.
    pub(crate) fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let length_sq = dx * dx + dy * dy;
        if length_sq < 1e-12 {
            let dx0 = ux - x1;
            let dy0 = uy - y1;
            return (dx0 * dx0 + dy0 * dy0).sqrt();
        }
        let t = ((ux - x1) * dx + (uy - y1) * dy) / length_sq;
        let t = t.clamp(0.0, 1.0);
        let px = x1 + t * dx;
        let py = y1 + t * dy;
        ((ux - px).powi(2) + (uy - py).powi(2)).sqrt()
    }

    /// Convert SDF value to pixel coverage with configurable antialiasing half-width.
    pub(crate) fn sdf_to_coverage_aa(sd: f32, aa_half: f32) -> f32 {
        ((aa_half - sd) / (2.0 * aa_half)).clamp(0.0, 1.0)
    }

    /// Convert SDF value to pixel coverage (AA half-width = 0.5).
    pub(crate) fn sdf_to_coverage(sd: f32) -> f32 {
        Self::sdf_to_coverage_aa(sd, 0.5)
    }

    // ── 阴影覆盖计算 ──

    /// Compute shadow coverage with smooth Gaussian-like falloff.
    pub(crate) fn shadow_coverage(sd: f32, blur: f32) -> f32 {
        let t = ((blur - sd) / (2.0 * blur)).clamp(0.0, 1.0);
        t * t * (3.0 - 2.0 * t)
    }

    /// Wider, softer shadow falloff for ambient layers (quintic smoothstep).
    pub(crate) fn shadow_coverage_ambient(sd: f32, blur: f32) -> f32 {
        let half = blur * 0.5;
        let t = ((half - sd) / (blur + half)).clamp(0.0, 1.0);
        let t2 = t * t;
        t2 * t2 * (5.0 - 4.0 * t)
    }
}

impl Canvas2D for CpuCanvas2D {
    // ═══════════════════════════════════════════
    // 矢量填充
    // ═══════════════════════════════════════════

    fn fill_rect(&mut self, rect: Rect, color: Color, radius: Option<Radius>) {
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
            let x0 = (rect.x + 0.5).floor() as i32;
            let y0 = (rect.y + 0.5).floor() as i32;
            let x1 = ((rect.x + rect.w) + 0.5).floor() as i32;
            let y1 = ((rect.y + rect.h) + 0.5).floor() as i32;
            let cw = x1 - x0;
            let ch = y1 - y0;
            if cw > 0 && ch > 0 {
                self.fill_rect_raw(x0, y0, cw, ch, c);
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
                        self.put_pixel_aa(px, py, c, coverage);
                    }
                }
            }
        }
    }

    fn fill_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color) {
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

    fn fill_ellipse(&mut self, rect: Rect, color: Color) {
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

    fn fill_sector(
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

    fn fill_path(&mut self, _path: &Path, _color: Color, _fill_rule: FillRule) {
        // TODO: path fill via rasterizer
    }

    // ═══════════════════════════════════════════
    // 矢量描边
    // ═══════════════════════════════════════════

    fn stroke_rect(&mut self, rect: Rect, color: Color, line_width: f32, radius: Option<Radius>) {
        let lw = line_width.max(0.0);
        self._stroke_rect_impl(rect, color, lw, radius.unwrap_or_default())
    }

    fn stroke_circle(&mut self, cx: f32, cy: f32, r: f32, color: Color, line_width: f32) {
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

    fn stroke_path(&mut self, _path: &Path, _color: Color, _opts: &StrokeOptions) {
        // TODO: path stroke
    }

    fn draw_line(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, color: Color, width: f32) {
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

    // ═══════════════════════════════════════════
    // 渐变
    // ═══════════════════════════════════════════

    fn fill_linear_gradient(
        &mut self,
        rect: Rect,
        color_a: Color,
        color_b: Color,
        dir: GradientDirection,
    ) {
        if let Some(cr) = self.intersect_clip(&rect) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let pixel_left = ux - 0.5;
                    let pixel_right = ux + 0.5;
                    let pixel_top = uy - 0.5;
                    let pixel_bottom = uy + 0.5;
                    let over_x = (pixel_right.min(rect.x + rect.w) - pixel_left.max(rect.x)).max(0.0);
                    let over_y = (pixel_bottom.min(rect.y + rect.h) - pixel_top.max(rect.y)).max(0.0);
                    let cover = over_x.min(over_y).min(1.0);
                    if cover <= 0.0 {
                        continue;
                    }
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
                    let blended = Self::apply_opacity_with(
                        self.opacity,
                        Self::premul(Color::from_rgba(r, g, b, a)),
                    );
                    self.put_pixel_aa(px, py, blended, cover);
                }
            }
        }
    }

    fn fill_radial_gradient(
        &mut self,
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
        let x0 = ((cx - outer_r).max(self.clip_rect.x)) as i32;
        let y0 = ((cy - outer_r).max(self.clip_rect.y)) as i32;
        let x1 = ((cx + outer_r).min(self.clip_rect.x + self.clip_rect.w)) as i32;
        let y1 = ((cy + outer_r).min(self.clip_rect.y + self.clip_rect.h)) as i32;
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
                let c = Self::apply_opacity_with(
                    self.opacity,
                    Self::premul(Color::from_rgba(r, g, b, a)),
                );
                let coverage = if range > 0.0 { 1.0 } else { Self::sdf_to_coverage(dist - outer_r) };
                self.put_pixel_aa(px, py, c, coverage);
            }
        }
    }

    // ═══════════════════════════════════════════
    // 阴影
    // ═══════════════════════════════════════════

    fn draw_box_shadow(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self._draw_box_shadow_impl(rect, blur_radius, offset_x, offset_y, color, corner_radius, false)
    }

    fn draw_box_shadow_ambient(
        &mut self,
        rect: Rect,
        blur_radius: f32,
        offset_x: f32,
        offset_y: f32,
        color: Color,
        corner_radius: Option<Radius>,
    ) {
        self._draw_box_shadow_impl(rect, blur_radius, offset_x, offset_y, color, corner_radius, true)
    }

    // ═══════════════════════════════════════════
    // 图像/字形混合
    // ═══════════════════════════════════════════

    fn blit_image(&mut self, src: &[u32], src_w: i32, src_rect: Rect, dst_rect: Rect) {
        if src_w <= 0 || src.is_empty() {
            return;
        }
        let src_h = (src.len() / src_w as usize) as i32;
        if src_h <= 0 {
            return;
        }
        let sx = src_rect.x.max(0.0) as i32;
        let sy = src_rect.y.max(0.0) as i32;
        let sw = (src_rect.w as i32).min(src_w - sx);
        let sh = (src_rect.h as i32).min(src_h - sy);
        if sw <= 0 || sh <= 0 {
            return;
        }
        let dx = dst_rect.x as i32;
        let dy = dst_rect.y as i32;
        let dw = dst_rect.w as i32;
        let dh = dst_rect.h as i32;

        if dw == sw && dh == sh {
            for row in 0..sh {
                for col in 0..sw {
                    let src_idx = ((sy + row) * src_w + (sx + col)) as usize;
                    if src_idx >= src.len() {
                        continue;
                    }
                    let p = Self::apply_opacity_with(self.opacity, src[src_idx]);
                    self.put_pixel_raw(dx + col, dy + row, p);
                }
            }
        } else {
            for row in 0..dh {
                for col in 0..dw {
                    let src_x = sx + (col * sw / dw);
                    let src_y = sy + (row * sh / dh);
                    let src_idx = (src_y * src_w + src_x) as usize;
                    if src_idx >= src.len() {
                        continue;
                    }
                    let p = Self::apply_opacity_with(self.opacity, src[src_idx]);
                    self.put_pixel_raw(dx + col, dy + row, p);
                }
            }
        }
    }

    fn blit_glyph(
        &mut self, x: i32, y: i32, coverage: &[u8],
        width: usize, height: usize, color: Color,
    ) {
        let premul = Self::premul(color);
        let surf_w = self.surface.width();
        let surf_h = self.surface.height();
        let (cx0, cy0, cx1, cy1) = self.clip_int;

        for row in 0..height {
            let py = y + row as i32;
            if py < cy0.max(0) || py >= cy1.min(surf_h) {
                continue;
            }
            for col in 0..width {
                let px = x + col as i32;
                if px < cx0.max(0) || px >= cx1.min(surf_w) {
                    continue;
                }
                let cov = coverage[row * width + col];
                if cov == 0 {
                    continue;
                }
                let alpha = ((premul >> 24) & 0xFF) as u8;
                if alpha == 0 {
                    continue;
                }
                let blended = Self::apply_opacity_with(self.opacity, premul);
                self.put_pixel_aa(px, py, blended, cov as f32 / 255.0);
            }
        }
    }

    // ═══════════════════════════════════════════
    // 渲染状态栈
    // ═══════════════════════════════════════════


    fn save(&mut self) {
        self.state_stack.push(StateSnapshot {
            clip_rect: self.clip_rect,
            clip_int: self.clip_int,
            opacity: self.opacity,
            transform: self.transform,
            invert: self.invert,
            blend_mode: self.blend_mode,
        });
    }

    fn restore(&mut self) {
        if let Some(snap) = self.state_stack.pop() {
            self.clip_rect = snap.clip_rect;
            self.clip_int = snap.clip_int;
            self.opacity = snap.opacity;
            self.transform = snap.transform;
            self.invert = snap.invert;
            self.blend_mode = snap.blend_mode;
        }
    }

    fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(self.clip_rect);
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
            self.sync_clip_int();
        } else {
            self.clip_rect = Rect::zero();
            self.clip_int = (0, 0, 0, 0);
        }
    }

    fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
            self.sync_clip_int();
        }
    }

    fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.max(0.0).min(1.0);
    }

    fn opacity(&self) -> f32 {
        self.opacity
    }

    fn set_transform(&mut self, t: Transform) {
        self.transform = t;
        self.invert = Self::compute_inverse(&t);
    }

    fn reset_transform(&mut self) {
        self.transform = Transform::identity();
        self.invert = Self::compute_inverse(&Transform::identity());
    }

    fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }

    fn push_clip_path(&mut self, _path: &Path) {
        // TODO: 路径裁剪实现
    }

    // ═══════════════════════════════════════════
    // 像素访问
    // ═══════════════════════════════════════════

    fn pixels_mut(&mut self) -> &mut [u32] {
        self.surface.pixels_mut()
    }

    fn surface_size(&self) -> uix_core::Size {
        self.surface.surface_size()
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 内部辅助方法（不在 Canvas2D trait 中）
// ════════════════════════════════════════════════════════════════════════════

impl CpuCanvas2D {
    pub(crate) fn _draw_box_shadow_impl(
        &mut self,
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

        let color = self.apply_opacity(Self::premul(shadow_color));
        let shadow_rect = Rect::new(rect.x + offset_x, rect.y + offset_y, rect.w, rect.h);

        let expand = blur + 1.0;
        let bounds = Rect::new(
            shadow_rect.x - expand,
            shadow_rect.y - expand,
            shadow_rect.w + expand * 2.0,
            shadow_rect.h + expand * 2.0,
        );

        let use_blur = blur > 0.5;

        if let Some(cr) = self.intersect_clip(&bounds) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;

            if x0 >= x1 || y0 >= y1 {
                return;
            }

            for py in y0..y1 {
                for px in x0..x1 {
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = Self::rounded_rect_sdf(ux, uy, &shadow_rect, &rad);

                    let coverage = if use_blur {
                        if ambient {
                            Self::shadow_coverage_ambient(sd, blur)
                        } else {
                            Self::shadow_coverage(sd, blur)
                        }
                    } else {
                        Self::sdf_to_coverage(sd)
                    };

                    if coverage > 0.0 {
                        self.put_pixel_aa(px, py, color, coverage);
                    }
                }
            }
        }
    }

    pub(crate) fn _stroke_rect_impl(&mut self, rect: Rect, color: Color, lw: f32, rad: Radius) {
        let c = self.apply_opacity(Self::premul(color));

        if rad.tl == 0.0 && rad.tr == 0.0 && rad.bl == 0.0 && rad.br == 0.0
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
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                    let coverage = Self::sdf_to_coverage(sd.abs() - h);
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
}
