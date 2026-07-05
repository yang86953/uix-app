//! RasterRenderer — 通用栅格渲染器。
//!
//! 持有渲染状态（裁剪、透明度、偏移、变换、混合模式），
//! 提供所有 Canvas2D 绘制方法的实现，但输出目标由调用者以像素缓冲传递。
//! CPU/GPU 后端只需「往哪写像素」，渲染逻辑由这里统一完成。

use crate::native::Rect;

use crate::draw::primitives::color::Color;
use crate::draw::primitives::types::{BlendMode, Transform};
use crate::draw::rasterizer::core as rast;

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
    pub(crate) clip_rect: Rect,
    /// 预计算的整数裁剪边界。
    clip_int: (i32, i32, i32, i32),
    /// 裁剪矩形栈。
    clip_stack: Vec<Rect>,
    /// 全局透明度。
    opacity: f32,
    /// 像素偏移量（画布平移）。
    pub(crate) offset_x: f32,
    pub(crate) offset_y: f32,
    /// 当前 2D 仿射变换。
    pub(crate) transform: Transform,
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

    pub(crate) fn apply_inverse(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.invert.map(|[a, b, tx, c, d, ty]| {
            let xf = x as f64;
            let yf = y as f64;
            ((a * xf + b * yf + tx) as f32, (c * xf + d * yf + ty) as f32)
        })
    }

    fn sync_clip_int(&mut self) {
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

    // ═══ 颜色工具 ═══

    pub(crate) fn premul(c: Color) -> u32 {
        rast::premul(c.to_rgba())
    }
    pub(crate) fn apply_opa(&self, c: u32) -> u32 {
        rast::apply_opacity(c, self.opacity)
    }

    // ═══ 像素操作 ═══

    fn blend_pixel(&self, pixels: &mut [u32], w: i32, h: i32, x: i32, y: i32, color: u32) {
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

    pub(crate) fn put_pixel_aa(
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
            self.blend_pixel(pixels, w, h, x, y, premul_color);
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
            self.blend_pixel(pixels, w, h, x + dx, y, color);
        }
    }

    pub(crate) fn fill_rect_raw(
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

    pub(crate) fn rounded_rect_sdf(
        ux: f32,
        uy: f32,
        r: &Rect,
        rad: &crate::draw::primitives::types::Radius,
    ) -> f32 {
        rast::rounded_rect_sdf(ux, uy, r, rad)
    }
    pub(crate) fn line_segment_sdf(ux: f32, uy: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
        rast::line_segment_sdf(ux, uy, x1, y1, x2, y2)
    }
    pub(crate) fn sdf_to_coverage(sd: f32) -> f32 {
        rast::sdf_to_coverage(sd)
    }
}

// draw_box_shadow 等由 Canvas2D trait 默认实现调用 rasterizer，
// RasterRenderer 不重复实现——默认方法已经够用。
