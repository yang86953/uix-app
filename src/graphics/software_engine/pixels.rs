use super::core::RenderTarget;
use crate::base::Rect;
use crate::graphics::Color;

// ════════════════════════════════════════════════════════════════════════════
// RenderTarget — 像素级操作
// ════════════════════════════════════════════════════════════════════════════

impl RenderTarget {
    pub fn put_pixel_aa(&mut self, x: i32, y: i32, premul_color: u32, coverage: f32) {
        let cx0 = self.clip_rect.x as i32;
        let cy0 = self.clip_rect.y as i32;
        let cx1 = (self.clip_rect.x + self.clip_rect.w).ceil() as i32;
        let cy1 = (self.clip_rect.y + self.clip_rect.h).ceil() as i32;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 { return; }

        if coverage >= 1.0 - 1e-6 {
            self.put_pixel_raw(x, y, premul_color);
            return;
        }
        if coverage <= 0.0 { return; }

        let idx = (y * self.width + x) as usize;

        let src_a = ((premul_color >> 24) & 0xFF) as f32;
        if src_a <= 0.0 { return; }

        let src_r_p = ((premul_color >> 16) & 0xFF) as f32 * coverage;
        let src_g_p = ((premul_color >> 8) & 0xFF) as f32 * coverage;
        let src_b_p = (premul_color & 0xFF) as f32 * coverage;
        let src_a_s = src_a * coverage;

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

        self.pixels[idx] = ((out_a.round() as u32).min(255) << 24)
            | ((out_r_p.round() as u32).min(255) << 16)
            | ((out_g_p.round() as u32).min(255) << 8)
            | (out_b_p.round() as u32).min(255);
    }

    pub fn put_pixel_raw(&mut self, x: i32, y: i32, color: u32) {
        let w = self.width;
        let h = self.height;
        if x < 0 || x >= w || y < 0 || y >= h { return; }
        let cx0 = self.clip_rect.x as i32;
        let cy0 = self.clip_rect.y as i32;
        let cx1 = (self.clip_rect.x + self.clip_rect.w).ceil() as i32;
        let cy1 = (self.clip_rect.y + self.clip_rect.h).ceil() as i32;
        if x < cx0 || y < cy0 || x >= cx1 || y >= cy1 { return; }
        let idx = (y * w + x) as usize;
        if idx >= self.pixels.len() { return; }
        let src_a = (color >> 24) & 0xFF;
        if src_a == 0 { return; }
        let dst = self.pixels[idx];
        let dst_a = (dst >> 24) & 0xFF;
        if src_a == 0xFF && dst_a == 0 {
            self.pixels[idx] = color;
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
        self.pixels[idx] = out_a << 24 | out_r << 16 | out_g << 8 | out_b;
    }

    pub(crate) fn fill_span(&mut self, x: i32, y: i32, w: i32, color: u32) {
        if (color >> 24) == 0xFF {
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
        let tw = self.width as i32;
        let x0 = x.max(0);
        let x1 = (x + w).min(tw);
        if y < 0 || y >= self.height as i32 || x0 >= x1 { return; }
        let start = (y * tw + x0) as usize;
        let count = (x1 - x0) as usize;
        let pixels = &mut self.pixels[start..start + count];

        let src_a = (color >> 24) & 0xFF;
        if src_a == 0 { return; }
        let src_r = (color >> 16) & 0xFF;
        let src_g = (color >> 8) & 0xFF;
        let src_b = color & 0xFF;

        for dst in pixels.iter_mut() {
            if *dst == 0 { *dst = color; continue; }
            let dst_a = (*dst >> 24) & 0xFF;
            let out_a = src_a + dst_a - (src_a * dst_a / 255);
            let dst_r = (*dst >> 16) & 0xFF;
            let dst_g = (*dst >> 8) & 0xFF;
            let dst_b = *dst & 0xFF;
            let out_r = src_r + (dst_r * (255 - src_a) / 255);
            let out_g = src_g + (dst_g * (255 - src_a) / 255);
            let out_b = src_b + (dst_b * (255 - src_a) / 255);
            *dst = (out_a << 24) | (out_r << 16) | (out_g << 8) | out_b;
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
        } else { None }
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

    /// Scroll (shift) pixel content within `viewport` by `dx` and `dy` pixels.
    /// dx>0 = 右滚（内容左移），dy>0 = 下滚（内容上移）。
    pub fn scroll_region(&mut self, viewport: Rect, dx: f32, dy: f32) {
        let vx = viewport.x as i32;
        let vy = viewport.y as i32;
        let vw = viewport.w as i32;
        let vh = viewport.h as i32;
        let stride = self.width;
        let ddx = dx.round() as i32;
        let ddy = dy.round() as i32;

        if (ddx == 0 && ddy == 0) || vw <= 0 || vh <= 0 { return; }

        // 垂直移位
        if ddy != 0 && ddy.abs() < vh {
            if ddy > 0 {
                for y in vy..vy + vh - ddy {
                    let dst = (y * stride + vx) as usize;
                    let src = ((y + ddy) * stride + vx) as usize;
                    self.pixels.copy_within(src..src + vw as usize, dst);
                }
            } else {
                let abs_d = -ddy;
                for y in (vy + abs_d..vy + vh).rev() {
                    let dst = (y * stride + vx) as usize;
                    let src = ((y - abs_d) * stride + vx) as usize;
                    self.pixels.copy_within(src..src + vw as usize, dst);
                }
            }
        }

        // 水平移位
        if ddx != 0 && ddx.abs() < vw {
            if ddx > 0 {
                for y in vy..vy + vh {
                    let base = (y * stride) as usize;
                    let dst = base + vx as usize;
                    let src = base + (vx + ddx) as usize;
                    let count = (vw - ddx) as usize;
                    self.pixels.copy_within(src..src + count, dst);
                }
            } else {
                let abs_d = -ddx;
                for y in vy..vy + vh {
                    let base = (y * stride) as usize;
                    let dst = base + (vx + abs_d) as usize;
                    let src = base + vx as usize;
                    let count = (vw - abs_d) as usize;
                    for i in (0..count).rev() {
                        self.pixels[dst + i] = self.pixels[src + i];
                    }
                }
            }
        }
    }
}
