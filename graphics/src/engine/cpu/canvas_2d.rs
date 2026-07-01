//! CPU 2D 绘制上下文——实现 Canvas2D trait。
//!
//! 组合 PixelSurface + 渲染状态栈。
//! 绘制方法委托 rasterizer 纯函数（待迁入），
//! 像素级操作直接访问底层 surface。

use uix_platform::Rect;

use crate::color::Color;
use crate::engine::cpu::pixel_surface::PixelSurface;
use crate::path::{FillRule, Path};
use crate::rasterizer::core as rast;
use crate::stroker::StrokeOptions;
use crate::traits::Canvas2D;
use crate::types::{BlendMode, Radius, Transform};

/// 渲染状态快照（用于 save/restore）。
#[derive(Clone)]
pub(crate) struct StateSnapshot {
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

    /// 设置内部 2D 变换（仅供 GPU 回退路径调用）。
    pub fn set_transform(&mut self, t: Transform) {
        self.transform = t;
        self.invert = Self::compute_inverse(&t);
    }

    /// 重置内部 2D 变换。
    pub fn reset_transform(&mut self) {
        self.transform = Transform::identity();
        self.invert = Self::compute_inverse(&Transform::identity());
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
        self.clip_int = rast::clip_to_int(&self.clip_rect);
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
        rast::intersect_rect(r, &self.clip_rect)
    }

    // ── 颜色工具（委托核心模块）──

    pub(crate) fn premul(c: Color) -> u32 {
        rast::premul(c.to_rgba())
    }

    pub(crate) fn apply_opacity(&self, c: u32) -> u32 {
        rast::apply_opacity(c, self.opacity)
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

    // ── SDF / 阴影工具（委托核心模块）──

    pub(crate) fn rounded_rect_sdf(ux: f32, uy: f32, r: &Rect, rad: &Radius) -> f32 {
        rast::rounded_rect_sdf(ux, uy, r, rad)
    }
    pub(crate) fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        rast::line_segment_sdf(ux, uy, x1, y1, x2, y2)
    }
    pub(crate) fn sdf_to_coverage(sd: f32) -> f32 {
        rast::sdf_to_coverage(sd)
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

    fn fill_path(&mut self, path: &Path, color: Color, fill_rule: FillRule) {
        let c = self.apply_opacity(Self::premul(color));
        let polys = crate::flattener::flatten(path.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        let w = self.surface.width();
        let h = self.surface.height();
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::polygon::fill_polygons(
            &polys, pixels, w, h, clip, c, fill_rule,
            &mut global_edges, &mut active_edges,
        );
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

    fn stroke_path(&mut self, path: &Path, color: Color, opts: &StrokeOptions) {
        let c = self.apply_opacity(Self::premul(color));
        let stroked = crate::stroker::stroke_path(path, opts);
        if stroked.is_empty() { return; }
        let polys = crate::flattener::flatten(stroked.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        let w = self.surface.width();
        let h = self.surface.height();
        let pixels = self.surface.pixels_mut();
        crate::rasterizer::polygon::fill_polygons(
            &polys, pixels, w, h, clip, c, crate::path::FillRule::NonZero,
            &mut global_edges, &mut active_edges,
        );
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
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    fn opacity(&self) -> f32 {
        self.opacity
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

    fn surface_size(&self) -> uix_platform::Size {
        self.surface.surface_size()
    }

    fn current_clip(&self) -> Rect {
        self.clip_rect
    }

    // ═══════════════════════════════════════════
    // 像素移动（滚动优化）
    // ═══════════════════════════════════════════

    fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        let int_dx = dx.round() as i32;
        let int_dy = dy.round() as i32;
        if int_dx == 0 && int_dy == 0 { return; }

        // 源区域：viewport 偏移 (-dx, -dy) 的内容是滚动后应该出现在 viewport 中的像素
        let src = Rect::new(
            viewport.x - dx,
            viewport.y - dy,
            viewport.w,
            viewport.h,
        );
        let dst_x = viewport.x as i32;
        let dst_y = viewport.y as i32;

        use crate::traits::RenderingBackend;
        RenderingBackend::copy_region(&mut self.surface, src, dst_x, dst_y);
    }
}
