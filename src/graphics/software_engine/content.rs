use super::core::RenderTarget;
use crate::graphics::{Color, GradientDirection};
use crate::base::{Point, Rect};

impl RenderTarget {
    pub fn fill_linear_gradient(
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

    pub fn fill_radial_gradient(
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
                let blended = Self::apply_opacity_with(
                    self.opacity,
                    Self::premul(Color::from_rgba(r, g, b, a)),
                );
                self.put_pixel_raw(px, py, blended);
            }
        }
    }

    // ── 位图字体（独立于 TextBackend）──

    pub fn draw_bitmap_text(&mut self, text: &str, pos: Point, color: Color) {
        let c = self.apply_opacity(Self::premul(color));
        let cw = 6i32;
        let lh = 9i32;
        let ox = pos.x as i32;
        let oy = pos.y as i32;
        let mut x = 0i32;
        let mut y = 0i32;
        for ch in text.chars() {
            if ch == '\n' {
                x = 0;
                y += lh;
                continue;
            }
            let idx = (ch as u8).wrapping_sub(32);
            if idx >= 95 {
                x += cw;
                continue;
            }
            let glyph = &crate::graphics::bitmap_font::FONT_CHARS[idx as usize];
            for (col, &byte) in glyph.iter().enumerate() {
                for row in 0..7usize {
                    if (byte >> row) & 1 != 0 {
                        self.put_pixel_raw(ox + x + col as i32, oy + y + row as i32, c);
                    }
                }
            }
            x += cw;
        }
    }

    // ── 图片 Blit ──

    pub(crate) fn apply_opacity_with(opacity: f32, color: u32) -> u32 {
        if (opacity - 1.0).abs() < 0.001 {
            return color;
        }
        let a = ((color >> 24) & 0xFF) as f32 * opacity;
        let r = ((color >> 16) & 0xFF) as f32 * opacity;
        let g = ((color >> 8) & 0xFF) as f32 * opacity;
        let b = (color & 0xFF) as f32 * opacity;
        ((a as u32).min(255) << 24)
            | ((r as u32).min(255) << 16)
            | ((g as u32).min(255) << 8)
            | (b as u32).min(255)
    }

    pub fn blit_image(
        &mut self,
        src_pixels: &[u32],
        src_w: i32,
        src_h: i32,
        src: &Rect,
        dst: &Rect,
        opacity: f32,
    ) {
        if src_w <= 0 || src_h <= 0 {
            return;
        }
        let sx = src.x.max(0.0) as i32;
        let sy = src.y.max(0.0) as i32;
        let sw = (src.w as i32).min(src_w - sx);
        let sh = (src.h as i32).min(src_h - sy);
        if sw <= 0 || sh <= 0 {
            return;
        }
        let dx = dst.x as i32;
        let dy = dst.y as i32;
        let dw = dst.w as i32;
        let dh = dst.h as i32;

        if dw == sw && dh == sh {
            for row in 0..sh {
                for col in 0..sw {
                    let src_idx = ((sy + row) * src_w + (sx + col)) as usize;
                    if src_idx >= src_pixels.len() {
                        continue;
                    }
                    let p = Self::apply_opacity_with(opacity, src_pixels[src_idx]);
                    self.put_pixel_raw(dx + col, dy + row, p);
                }
            }
        } else {
            for row in 0..dh {
                for col in 0..dw {
                    let src_x = sx + (col * sw / dw);
                    let src_y = sy + (row * sh / dh);
                    let src_idx = (src_y * src_w + src_x) as usize;
                    if src_idx >= src_pixels.len() {
                        continue;
                    }
                    let p = Self::apply_opacity_with(opacity, src_pixels[src_idx]);
                    self.put_pixel_raw(dx + col, dy + row, p);
                }
            }
        }
    }
}
