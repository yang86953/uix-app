use super::core::RenderTarget;
use super::FontData;
use crate::graphics::VAlign;
use crate::graphics::{Color, GradientDirection, Point, Rect, Size, TextLayoutOptions};

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
                    let cov_x = if ux >= rect.x && ux <= rect.x + rect.w - 1.0 {
                        1.0
                    } else if ux < rect.x {
                        (ux + 0.5 - rect.x).clamp(0.0, 1.0)
                    } else {
                        (rect.x + rect.w - (ux - 0.5)).clamp(0.0, 1.0)
                    };
                    let cov_y = if uy >= rect.y && uy <= rect.y + rect.h - 1.0 {
                        1.0
                    } else if uy < rect.y {
                        (uy + 0.5 - rect.y).clamp(0.0, 1.0)
                    } else {
                        (rect.y + rect.h - (uy - 0.5)).clamp(0.0, 1.0)
                    };
                    let cover = cov_x.min(cov_y).clamp(0.0, 1.0);
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

    // ── TTF 文本渲染 ──

    pub(crate) fn measure_ttf(font: &FontData, text: &str, opts: &TextLayoutOptions) -> Size {
        if text.is_empty() {
            return Size::new(0.0, 0.0);
        }
        let lh = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            font.font
                .horizontal_line_metrics(font.size)
                .map(|m| m.new_line_size)
                .unwrap_or(font.size * 1.3)
        };
        let max_w = if opts.max_width.is_finite() && opts.max_width > 0.0 {
            opts.max_width
        } else {
            f32::MAX
        };

        if opts.word_wrap {
            let mut line_w = 0.0f32;
            let mut max_line = 0.0f32;
            let mut lines = 1u32;
            for ch in text.chars() {
                if ch == '\n' {
                    lines += 1;
                    max_line = max_line.max(line_w);
                    line_w = 0.0;
                    continue;
                }
                let advance = font.char_advance(ch);
                if line_w + advance > max_w && line_w > 0.0 {
                    lines += 1;
                    max_line = max_line.max(line_w);
                    line_w = advance;
                } else {
                    line_w += advance;
                }
            }
            max_line = max_line.max(line_w);
            Size::new(max_line.min(max_w), lines as f32 * lh)
        } else {
            let mut w = 0.0f32;
            let mut lines = 1u32;
            for ch in text.chars() {
                if ch == '\n' {
                    lines += 1;
                } else {
                    w += font.char_advance(ch);
                }
            }
            Size::new(w.min(max_w), lines as f32 * lh)
        }
    }

    pub(crate) fn draw_ttf(
        &mut self,
        font: &FontData,
        text: &str,
        pos: Point,
        color: Color,
        opts: &TextLayoutOptions,
    ) {
        let c = self.apply_opacity(Self::premul(color));
        let lh = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            font.font
                .horizontal_line_metrics(font.size)
                .map(|m| m.new_line_size)
                .unwrap_or(font.size * 1.3)
        };
        let max_w = if opts.max_width.is_finite() && opts.max_width > 0.0 {
            opts.max_width
        } else {
            f32::MAX
        };

        struct GlyphPos {
            ch: char,
            x: f32,
            y: f32,
        }
        let mut glyphs: Vec<GlyphPos> = Vec::new();
        let mut cursor_x = 0.0f32;
        let mut cursor_y = 0.0f32;

        for ch in text.chars() {
            if ch == '\n' {
                cursor_x = 0.0;
                cursor_y += lh;
                continue;
            }
            let advance = font.char_advance(ch);
            if opts.word_wrap && cursor_x + advance > max_w && cursor_x > 0.0 {
                cursor_x = 0.0;
                cursor_y += lh;
            }
            glyphs.push(GlyphPos {
                ch,
                x: cursor_x,
                y: cursor_y,
            });
            cursor_x += advance;
        }
        if glyphs.is_empty() {
            return;
        }

        let total_h = cursor_y + lh;
        let vy_offset = match opts.v_align {
            VAlign::Top => 0.0,
            VAlign::Middle => -total_h / 2.0,
            VAlign::Bottom => -total_h,
            VAlign::Baseline => 0.0,
        };

        for gp in &glyphs {
            let (metrics, coverage) = font.font.rasterize(gp.ch, font.size);
            let gx = (pos.x + gp.x + metrics.xmin as f32) as i32;
            let gy = (pos.y + gp.y + metrics.ymin as f32 + vy_offset) as i32;
            for row in 0..metrics.height {
                for col in 0..metrics.width {
                    let cov = coverage[row * metrics.width + col];
                    if cov == 0 {
                        continue;
                    }
                    let alpha = (c >> 24) & 0xFF;
                    let cov_u32 = cov as u32;
                    let blended_alpha = (alpha * cov_u32 / 255).min(255);
                    let r = (c & 0xFF) * cov_u32 / 255;
                    let g = ((c >> 8) & 0xFF) * cov_u32 / 255;
                    let b = ((c >> 16) & 0xFF) * cov_u32 / 255;
                    let pixel = (blended_alpha << 24) | (b << 16) | (g << 8) | r;
                    self.put_pixel_raw(gx + col as i32, gy + row as i32, pixel);
                }
            }
        }
    }

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
        let r = (color & 0xFF) as f32 * opacity;
        let g = ((color >> 8) & 0xFF) as f32 * opacity;
        let b = ((color >> 16) & 0xFF) as f32 * opacity;
        ((a as u32).min(255) << 24)
            | ((b as u32).min(255) << 16)
            | ((g as u32).min(255) << 8)
            | (r as u32).min(255)
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
