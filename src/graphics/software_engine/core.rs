use super::*;
use crate::graphics::{Transform};
use crate::base::{Rect};

// ════════════════════════════════════════════════════════════════════════════
// RenderTarget — holds all mutable render state
// ════════════════════════════════════════════════════════════════════════════

pub struct RenderTarget {
    pub(crate) pixels: Vec<u32>,
    pub(crate) width: i32,
    pub(crate) height: i32,
    pub(crate) clip_rect: Rect,
    clip_stack: Vec<Rect>,
    pub(crate) opacity: f32,
    pub(crate) transform: Transform,
    invert: Option<[f64; 6]>,
    blend_mode: BlendMode,
    state_stack: Vec<RenderState>,
    // Supersample level (0 = use adaptive default; allowed 1..=8)
    pub(crate) supersample_level: u8,
}

impl RenderTarget {
    pub fn new() -> Self {
        Self {
            pixels: Vec::new(),
            width: 0,
            height: 0,
            clip_rect: Rect::new(0.0, 0.0, f32::MAX, f32::MAX),
            clip_stack: Vec::new(),
            opacity: 1.0,
            transform: Transform::identity(),
            invert: None,
            blend_mode: BlendMode::Alpha,
            state_stack: Vec::new(),
            supersample_level: 0,
        }
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 缓冲访问
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    pub fn pixels(&self) -> &[u32] { &self.pixels }
    pub fn pixel_buffer(&self) -> &[u32] { &self.pixels }
    pub fn pixel_buffer_mut(&mut self) -> &mut [u32] { &mut self.pixels }
    pub fn pixel_bytes(&self) -> &[u8] {
        crate::graphics::software_engine::core::slice_u32_as_u8(&self.pixels)
    }
    pub fn width(&self) -> i32 { self.width }
    pub fn height(&self) -> i32 { self.height }
    pub fn opacity_val(&self) -> f32 { self.opacity }
    pub fn set_supersample_level(&mut self, level: u8) {
        self.supersample_level = level.min(8);
    }
    pub fn supersample_level(&self) -> u8 { self.supersample_level }
    pub fn clip_rect(&self) -> Rect { self.clip_rect }
    pub fn clip_rect_mut(&mut self) -> &mut Rect { &mut self.clip_rect }
    pub fn clip_stack_mut(&mut self) -> &mut Vec<Rect> { &mut self.clip_stack }

    pub fn initialize(&mut self, width: i32, height: i32) {
        self.width = width;
        self.height = height;
        self.pixels = vec![0x00000000; (width * height) as usize];
        self.clip_rect = Rect::new(0.0, 0.0, width as f32, height as f32);
        self.clip_stack.clear();
        self.state_stack.clear();
        self.invert = Self::compute_inverse(&self.transform);
    }

    pub fn take_pixels(&mut self) -> Vec<u32> {
        std::mem::take(&mut self.pixels)
    }

    pub fn set_pixels(&mut self, pixels: Vec<u32>, w: i32, h: i32) {
        self.width = w;
        self.height = h;
        self.pixels = pixels;
    }
}

// ════════════════════════════════════════════════════════════════════════════
// 变换工具
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    pub fn is_identity(t: &Transform) -> bool {
        t.m[0] == 1.0 && t.m[1] == 0.0 && t.m[2] == 0.0
            && t.m[3] == 0.0 && t.m[4] == 1.0 && t.m[5] == 0.0
    }

    fn compute_inverse(t: &Transform) -> Option<[f64; 6]> {
        let [a, b, tx, c, d, ty] = t.m.map(|v| v as f64);
        let det = a * d - b * c;
        if det.abs() < 1e-12 { return None; }
        let inv = 1.0 / det;
        Some([
            inv * d, inv * (-b), inv * (b * ty - d * tx),
            inv * (-c), inv * a, inv * (c * tx - a * ty),
        ])
    }

    fn apply_transform(&self, x: f32, y: f32) -> (f32, f32) {
        let [a, b, tx, c, d, ty] = self.transform.m;
        (a * x + b * y + tx, c * x + d * y + ty)
    }

    pub fn apply_inverse(&self, x: f32, y: f32) -> Option<(f32, f32)> {
        self.invert.map(|[a, b, tx, c, d, ty]| {
            let xf = x as f64;
            let yf = y as f64;
            ((a * xf + b * yf + tx) as f32, (c * xf + d * yf + ty) as f32)
        })
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
}

// ════════════════════════════════════════════════════════════════════════════
// Clip / 状态 / 混合
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
<<<<<<< Updated upstream
    pub fn put_pixel_aa(&mut self, x: i32, y: i32, premul_color: u32, coverage: f32) {
        // Respect clip rect using integer bounds (avoid float precision issues)
        let cx0 = self.clip_rect.x as i32;
        let cy0 = self.clip_rect.y as i32;
        let cx1 = (self.clip_rect.x + self.clip_rect.w).ceil() as i32;
        let cy1 = (self.clip_rect.y + self.clip_rect.h).ceil() as i32;
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

        // Use float compositing with premultiplied channels scaled by coverage
        // to avoid coarse quantization for very thin strokes (e.g. 0.5px).
        let src_a = ((premul_color >> 24) & 0xFF) as f32;
        if src_a <= 0.0 {
            return;
        }

        // ARGB format: (A<<24) | (R<<16) | (G<<8) | B
        let src_r_p = ((premul_color >> 16) & 0xFF) as f32 * coverage;
        let src_g_p = ((premul_color >> 8) & 0xFF) as f32 * coverage;
        let src_b_p = (premul_color & 0xFF) as f32 * coverage;
        let src_a_s = src_a * coverage;

        let w = self.width;
        let h = self.height;
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        let idx = (y * w + x) as usize;
        if idx >= self.pixels.len() {
            return;
        }

        let dst = self.pixels[idx];
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

        self.pixels[idx] = (out_a_u.min(255) << 24)
            | (out_r_u.min(255) << 16)
            | (out_g_u.min(255) << 8)
            | out_b_u.min(255);
    }

    pub fn put_pixel_raw(&mut self, x: i32, y: i32, color: u32) {
        let w = self.width;
        let h = self.height;
        if x < 0 || x >= w || y < 0 || y >= h {
            return;
        }
        // Respect clip rect using integer bounds (avoid float precision issues)
        let cx0 = self.clip_rect.x as i32;
        let cy0 = self.clip_rect.y as i32;
        let cx1 = (self.clip_rect.x + self.clip_rect.w).ceil() as i32;
        let cy1 = (self.clip_rect.y + self.clip_rect.h).ceil() as i32;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 {
            return;
        }
        let idx = (y * w + x) as usize;
        if idx >= self.pixels.len() {
            return;
        }
        let src_a = (color >> 24) & 0xFF;
        if src_a == 0 {
            return;
        }
        let dst = self.pixels[idx];
        let dst_a = (dst >> 24) & 0xFF;
        if src_a == 0xFF && dst_a == 0 {
            self.pixels[idx] = color;
            return;
        }
        // ARGB format: byte[3]=A, byte[2]=R, byte[1]=G, byte[0]=B (LE: B,G,R,A)
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
        self.pixels[idx] = out_a << 24 | out_r << 16 | out_g << 8 | out_b;
    }

    pub(crate) fn fill_span(&mut self, x: i32, y: i32, w: i32, color: u32) {
        // 快速路径：不透明填充直接用 slice::fill 替代逐像素 put_pixel_raw。
        // 当 alpha=255 时，混合结果 = 源色，slice::fill 语义等价。
        if (color >> 24) == 0xFF {
            // 裁剪到裁剪矩形 + 边界（clip_rect 保证在 buffer 范围内）
            let tw = self.width as i32;
            let th = self.height as i32;
            let cx0 = self.clip_rect.x as i32;
            let cy0 = self.clip_rect.y as i32;
            let cx1 = (self.clip_rect.x + self.clip_rect.w).ceil() as i32;
            let cy1 = (self.clip_rect.y + self.clip_rect.h).ceil() as i32;
            let x0 = x.max(cx0).max(0);
            let x1 = (x + w).min(cx1).min(tw);
            if y >= cy0.max(0) && y < cy1.min(th) && x0 < x1 {
                let start = (y * tw + x0) as usize;
                self.pixels[start..start + (x1 - x0) as usize].fill(color);
            }
            return;
        }
        // 慢速路径：半透明填充逐像素执行 alpha 混合
        for dx in 0..w {
            self.put_pixel_raw(x + dx, y, color);
        }
    }

    pub fn fill_rect_raw(&mut self, x: i32, y: i32, w: i32, h: i32, color: u32) {
        for dy in 0..h {
            self.fill_span(x, y + dy, w, color);
        }
    }

    pub fn premul(c: Color) -> u32 {
        let a = c.a as u32;
        let r = (c.r as u32 * a / 255).min(255);
        let g = (c.g as u32 * a / 255).min(255);
        let b = (c.b as u32 * a / 255).min(255);
        (a << 24) | (r << 16) | (g << 8) | b
    }

    pub fn apply_opacity(&self, c: u32) -> u32 {
        let a = ((c >> 24) & 0xFF) as f32 * self.opacity;
        let r = ((c >> 16) & 0xFF) as f32 * self.opacity;
        let g = ((c >> 8) & 0xFF) as f32 * self.opacity;
        let b = (c & 0xFF) as f32 * self.opacity;
        ((a as u32).min(255) << 24)
            | ((r as u32).min(255) << 16)
            | ((g as u32).min(255) << 8)
            | (b as u32).min(255)
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

    #[allow(dead_code)]
    fn intersect_ranges(&self, r: &Rect) -> Option<(i32, i32, i32, i32)> {
        let x0 = (r.x.max(self.clip_rect.x)) as i32;
        let y0 = (r.y.max(self.clip_rect.y)) as i32;
        let x1 = ((r.x + r.w).min(self.clip_rect.x + self.clip_rect.w)) as i32;
        let y1 = ((r.y + r.h).min(self.clip_rect.y + self.clip_rect.h)) as i32;
        if x0 < x1 && y0 < y1 {
            Some((x0, y0, x1, y1))
        } else {
            None
        }
    }

    pub fn clear_all(&mut self) {
        self.pixels.fill(0x00000000);
    }

    pub fn clear_region(&mut self, x: i32, y: i32, w: i32, h: i32) {
        let tw = self.width;
        for row in 0..h {
            let start = ((y + row) * tw + x) as usize;
            self.pixels[start..start + w as usize].fill(0x00000000);
        }
    }

    /// Scroll (shift) pixel content within `viewport` by `dy` pixels.
    ///
    /// `dy > 0` = 滚动向下（内容上移），`dy < 0` = 滚动向上（内容下移）。
    /// 先复制行（memmove），之后调用方只需清除并渲染新增 strip。
    pub fn scroll_region(&mut self, viewport: Rect, dy: f32) {
        let vx = viewport.x as i32;
        let vy = viewport.y as i32;
        let vw = viewport.w as i32;
        let vh = viewport.h as i32;
        let stride = self.width;
        let delta = dy.round() as i32;

        if delta == 0 || delta.abs() >= vh || vw <= 0 || vh <= 0 {
            return;
        }

        if delta > 0 {
            // 向下滚动：内容上移 | 复制 [vy+delta..vy+vh) → [vy..vy+vh-delta)
            // 方向：从源到目标正向复制（源 > 目标，无重叠风险）
            for y in vy..vy + vh - delta {
                let dst = (y * stride + vx) as usize;
                let src = ((y + delta) * stride + vx) as usize;
                self.pixels.copy_within(src..src + vw as usize, dst);
            }
        } else {
            // 向上滚动：内容下移 | 复制 [vy..vy+vh+delta) → [vy-delta..vy+vh)
            // 方向：从源到目标反向复制（源 < 目标，需逆序遍历防止覆盖）
            let abs_d = -delta;
            for y in (vy + abs_d..vy + vh).rev() {
                let dst = (y * stride + vx) as usize;
                let src = ((y - abs_d) * stride + vx) as usize;
                self.pixels.copy_within(src..src + vw as usize, dst);
            }
        }
    }

=======
>>>>>>> Stashed changes
    pub fn push_clip_rect(&mut self, rect: Rect) {
        self.clip_stack.push(self.clip_rect);
        let cur = self.clip_rect;
        let x = rect.x.max(cur.x);
        let y = rect.y.max(cur.y);
        let r = (rect.x + rect.w).min(cur.x + cur.w);
        let b = (rect.y + rect.h).min(cur.y + cur.h);
        self.clip_rect = Rect::new(x, y, (r - x).max(0.0), (b - y).max(0.0));
    }

    pub fn pop_clip_rect(&mut self) {
        self.clip_rect = self.clip_stack.pop()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, self.width as f32, self.height as f32));
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.opacity = opacity;
    }

    pub fn save(&mut self) {
        self.state_stack.push(RenderState {
            clip_rect: self.clip_rect,
            opacity: self.opacity,
            transform: self.transform,
            blend_mode: self.blend_mode,
        });
    }

    pub fn restore(&mut self) {
        if let Some(state) = self.state_stack.pop() {
            self.clip_rect = state.clip_rect;
            self.opacity = state.opacity;
            self.transform = state.transform;
            self.invert = Self::compute_inverse(&state.transform);
            self.blend_mode = state.blend_mode;
        }
    }

    pub fn set_transform(&mut self, t: Transform) {
        self.transform = t;
        self.invert = Self::compute_inverse(&self.transform);
    }

    pub fn reset_transform(&mut self) {
        self.transform = Transform::identity();
        self.invert = Self::compute_inverse(&Transform::identity());
    }

    pub fn set_blend_mode(&mut self, mode: BlendMode) {
        self.blend_mode = mode;
    }
}

/// Reinterprets a `&[u32]` as `&[u8]` with 4x the length.
pub fn slice_u32_as_u8(slice: &[u32]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(slice.as_ptr() as *const u8, slice.len() * 4) }
}
