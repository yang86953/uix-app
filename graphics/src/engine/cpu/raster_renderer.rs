//! RasterRenderer — 通用栅格渲染器。
//!
//! 持有渲染状态（裁剪、透明度、偏移、变换、混合模式），
//! 提供所有 Canvas2D 绘制方法的实现，但输出目标由调用者以像素缓冲传递。
//! CPU/GPU 后端只需「往哪写像素」，渲染逻辑由这里统一完成。

use uix_platform::Rect;

use crate::color::Color;
use crate::path::{FillRule, Path};
use crate::rasterizer::core as rast;
use crate::stroker::StrokeOptions;
use crate::types::{BlendMode, Radius, Transform};

/// 渲染状态快照（用于 save/restore）。
#[derive(Clone)]
struct StateSnapshot {
    clip_rect: Rect,
    clip_int: (i32, i32, i32, i32),
    opacity: f32,
    offset_x: f32,
    offset_y: f32,
    transform: Transform,
    invert: Option<[f64; 6]>,
    blend_mode: BlendMode,
}

/// 通用栅格渲染器。
///
/// 管理所有渲染状态，绘制方法写入调用者传入的像素缓冲。
pub struct RasterRenderer {
    /// 当前裁剪矩形（浮点）。
    clip_rect: Rect,
    /// 预计算的整数裁剪边界。
    clip_int: (i32, i32, i32, i32),
    /// 裁剪矩形栈。
    clip_stack: Vec<Rect>,
    /// 全局透明度。
    opacity: f32,
    /// 像素偏移量（画布平移）。
    offset_x: f32,
    offset_y: f32,
    /// 当前 2D 仿射变换。
    transform: Transform,
    /// 逆变换缓存。
    invert: Option<[f64; 6]>,
    /// 混合模式。
    blend_mode: BlendMode,
    /// 状态快照栈。
    state_stack: Vec<StateSnapshot>,
}

impl RasterRenderer {
    /// 创建新渲染器，默认全屏裁剪、identity 变换。
    pub fn new(surface_w: i32, surface_h: i32) -> Self {
        Self {
            clip_rect: Rect::new(0.0, 0.0, surface_w as f32, surface_h as f32),
            clip_int: (0, 0, surface_w, surface_h),
            clip_stack: Vec::new(),
            opacity: 1.0,
            offset_x: 0.0,
            offset_y: 0.0,
            transform: Transform::identity(),
            invert: Self::compute_inverse(&Transform::identity()),
            blend_mode: BlendMode::default(),
            state_stack: Vec::new(),
        }
    }

    // ═══ 状态访问器 ═══

    pub fn clip_rect(&self) -> Rect {
        self.clip_rect
    }
    pub fn opacity(&self) -> f32 {
        self.opacity
    }
    pub fn offset(&self) -> (f32, f32) {
        (self.offset_x, self.offset_y)
    }
    pub fn set_offset(&mut self, dx: f32, dy: f32) {
        self.offset_x = dx;
        self.offset_y = dy;
    }
    pub fn set_transform(&mut self, t: Transform) {
        self.transform = t;
        self.invert = Self::compute_inverse(&t);
    }

    // ═══ 状态管理 ═══

    pub fn save(&mut self) {
        self.state_stack.push(StateSnapshot {
            clip_rect: self.clip_rect,
            clip_int: self.clip_int,
            opacity: self.opacity,
            offset_x: self.offset_x,
            offset_y: self.offset_y,
            transform: self.transform,
            invert: self.invert,
            blend_mode: self.blend_mode,
        });
    }

    pub fn restore(&mut self) {
        if let Some(snap) = self.state_stack.pop() {
            self.clip_rect = snap.clip_rect;
            self.clip_int = snap.clip_int;
            self.opacity = snap.opacity;
            self.offset_x = snap.offset_x;
            self.offset_y = snap.offset_y;
            self.transform = snap.transform;
            self.invert = snap.invert;
            self.blend_mode = snap.blend_mode;
        }
    }

    pub fn push_clip(&mut self, rect: Rect) {
        self.clip_stack.push(self.clip_rect);
        if let Some(intersection) = self.clip_rect.intersect(&rect) {
            self.clip_rect = intersection;
            self.sync_clip_int();
        } else {
            self.clip_rect = Rect::zero();
            self.clip_int = (0, 0, 0, 0);
        }
    }

    pub fn pop_clip(&mut self) {
        if let Some(prev) = self.clip_stack.pop() {
            self.clip_rect = prev;
            self.sync_clip_int();
        }
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    // ═══ 变换工具 ═══

    pub(crate) fn is_identity(t: &Transform) -> bool {
        t.m[0] == 1.0
            && t.m[1] == 0.0
            && t.m[2] == 0.0
            && t.m[3] == 0.0
            && t.m[4] == 1.0
            && t.m[5] == 0.0
    }

    fn compute_inverse(t: &Transform) -> Option<[f64; 6]> {
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

    fn apply_transform(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, tx, c, d, ty] = self.transform.m;
        (a * x + b * y + tx, c * x + d * y + ty)
    }

    fn apply_inverse(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.invert.map(|[a, b, tx, c, d, ty]| {
            let xf = x as f64;
            let yf = y as f64;
            ((a * xf + b * yf + tx) as f32, (c * xf + d * yf + ty) as f32)
        })
    }

    fn sync_clip_int(&mut self) {
        self.clip_int = rast::clip_to_int(&self.clip_rect);
    }

    fn transform_rect(&self, r: &Rect) -> Rect {
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

    fn intersect_clip(&self, r: &Rect) -> Option<Rect> {
        rast::intersect_rect(r, &self.clip_rect)
    }

    // ═══ 颜色工具 ═══

    fn premul(c: Color) -> u32 {
        rast::premul(c.to_rgba())
    }
    fn apply_opa(&self, c: u32) -> u32 {
        rast::apply_opacity(c, self.opacity)
    }

    // ═══ 像素操作 ═══

    fn put_pixel_raw(&self, pixels: &mut [u32], w: i32, h: i32, x: i32, y: i32, color: u32) {
        let (cx0, cy0, cx1, cy1) = self.clip_int;
        if x < cx0.max(0) || y < cy0.max(0) || x >= cx1.min(w) || y >= cy1.min(h) {
            return;
        }
        let idx = (y * w + x) as usize;
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

    fn put_pixel_aa(
        &self,
        pixels: &mut [u32],
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        premul_color: u32,
        coverage: f32,
    ) {
        let (cx0, cy0, cx1, cy1) = self.clip_int;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 {
            return;
        }
        if coverage >= 1.0 - 1e-6 {
            self.put_pixel_raw(pixels, w, h, x, y, premul_color);
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
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
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
        pixels[idx] = (out_a.round() as u32).min(255) << 24
            | (out_r_p.round() as u32).min(255) << 16
            | (out_g_p.round() as u32).min(255) << 8
            | (out_b_p.round() as u32).min(255);
    }

    fn fill_span(
        &self,
        pixels: &mut [u32],
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        span_w: i32,
        color: u32,
    ) {
        if (color >> 24) == 0xFF {
            let (cx0, cy0, cx1, cy1) = self.clip_int;
            let x0 = x.max(cx0).max(0);
            let x1 = (x + span_w).min(cx1).min(w);
            if y >= cy0.max(0) && y < cy1.min(h) && x0 < x1 {
                let start = (y * w + x0) as usize;
                pixels[start..start + (x1 - x0) as usize].fill(color);
            }
            return;
        }
        for dx in 0..span_w {
            self.put_pixel_raw(pixels, w, h, x + dx, y, color);
        }
    }

    fn fill_rect_raw(
        &self,
        pixels: &mut [u32],
        w: i32,
        h: i32,
        x: i32,
        y: i32,
        rw: i32,
        rh: i32,
        color: u32,
    ) {
        for dy in 0..rh {
            self.fill_span(pixels, w, h, x, y + dy, rw, color);
        }
    }

    // ═══ SDF 工具 ═══

    fn rounded_rect_sdf(ux: f32, uy: f32, r: &Rect, rad: &Radius) -> f32 {
        rast::rounded_rect_sdf(ux, uy, r, rad)
    }
    fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        rast::line_segment_sdf(ux, uy, x1, y1, x2, y2)
    }
    fn sdf_to_coverage(sd: f32) -> f32 {
        rast::sdf_to_coverage(sd)
    }

    // ═══════════════════════════════════════════
    // 矢量填充
    // ═══════════════════════════════════════════

    pub fn fill_rect(
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

    pub fn fill_circle(
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

    pub fn fill_ellipse(
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
        if let Some(cr) = self.intersect_clip(&expanded) {
            let x0 = cr.x as i32;
            let y0 = cr.y as i32;
            let x1 = (cr.x + cr.w) as i32;
            let y1 = (cr.y + cr.h) as i32;
            for py in y0..y1 {
                for px in x0..x1 {
                    let dx = px as f32 + 0.5 - cx;
                    let dy = py as f32 + 0.5 - cy;
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

    pub fn fill_sector(
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

    pub fn fill_path(
        &self,
        pixels: &mut [u32],
        surface_w: i32,
        surface_h: i32,
        path: &Path,
        color: Color,
        fill_rule: FillRule,
    ) {
        let (ox, oy) = (self.offset_x, self.offset_y);
        let path = if ox != 0.0 || oy != 0.0 {
            &path.translated(ox, oy)
        } else {
            path
        };
        let c = self.apply_opa(Self::premul(color));
        let polys = crate::flattener::flatten(path.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        crate::rasterizer::polygon::fill_polygons(
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

    // ═══════════════════════════════════════════
    // 矢量描边
    // ═══════════════════════════════════════════

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
                    let ux = px as f32 + 0.5;
                    let uy = py as f32 + 0.5;
                    let sd = Self::rounded_rect_sdf(ux, uy, &rect, &rad);
                    let coverage = Self::sdf_to_coverage(sd.abs() - h);
                    if coverage > 0.0 {
                        self.put_pixel_aa(pixels, surface_w, surface_h, px, py, c, coverage);
                    }
                }
            }
        }
    }

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
        let stroked = crate::stroker::stroke_path(path, opts);
        if stroked.is_empty() {
            return;
        }
        let polys = crate::flattener::flatten(stroked.segments(), 0.25);
        let mut global_edges = Vec::new();
        let mut active_edges = Vec::new();
        let clip = self.clip_rect;
        crate::rasterizer::polygon::fill_polygons(
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

    // ═══════════════════════════════════════════
    // 阴影（委托 rasterizer 默认实现）
    // ═══════════════════════════════════════════

    // draw_box_shadow 等由 Canvas2D trait 默认实现调用 rasterizer，
    // RasterRenderer 不重复实现——默认方法已经够用。
}
