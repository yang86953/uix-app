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

    // ── TTF 文本渲染（使用 fontdue Layout 引擎）──

    /// 将 TextLayoutOptions 转换为 fontdue LayoutSettings 并运行布局
    fn run_layout<'a>(font: &'a FontData, text: &str, opts: &TextLayoutOptions, pos_x: f32, pos_y: f32)
        -> fontdue::layout::Layout<()>
    {
        use fontdue::layout::*;
        let fs = opts.font_size.max(1.0);
        let new_line_size = font.font.horizontal_line_metrics(fs)
            .map(|m| m.new_line_size).unwrap_or(fs * 1.3);
        let lh_abs = if opts.line_height > 0.0 { opts.line_height } else { new_line_size };
        let lh_mult = lh_abs / new_line_size;
        let max_w = if opts.max_width.is_finite() && opts.max_width > 0.0 {
            Some(opts.max_width) } else { None };

        let h_align = match opts.h_align {
            crate::graphics::HAlign::Left => HorizontalAlign::Left,
            crate::graphics::HAlign::Center => HorizontalAlign::Center,
            crate::graphics::HAlign::Right => HorizontalAlign::Right,
        };
        let v_align = match opts.v_align {
            VAlign::Top => VerticalAlign::Top,
            VAlign::Middle => VerticalAlign::Middle,
            VAlign::Bottom => VerticalAlign::Bottom,
            VAlign::Baseline => VerticalAlign::Top, // fontdue Layout doesn't have Baseline
        };
        let wrap = if opts.word_wrap { WrapStyle::Word } else { WrapStyle::Letter };

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: pos_x, y: pos_y,
            max_width: max_w,
            max_height: None,
            horizontal_align: h_align,
            vertical_align: v_align,
            line_height: lh_mult,
            wrap_style: wrap,
            wrap_hard_breaks: true,
        });
        layout.append(&[&font.font], &TextStyle::new(text, fs, 0));
        layout
    }

    pub(crate) fn measure_ttf(font: &FontData, text: &str, opts: &TextLayoutOptions) -> Size {
        if text.is_empty() {
            return Size::new(0.0, 0.0);
        }
        let layout = Self::run_layout(font, text, opts, 0.0, 0.0);
        let gw = layout.glyphs();
        let max_x = gw.iter().fold(0.0f32, |m, g| (g.x + g.width as f32).max(m));
        let h = layout.height().max(0.0);
        Size::new(max_x, h)
    }

    pub(crate) fn draw_ttf(
        &mut self,
        font: &FontData,
        text: &str,
        pos: Point,
        color: Color,
        opts: &TextLayoutOptions,
    ) {
        if text.is_empty() { return; }
        let fs = opts.font_size.max(1.0);
        let c = self.apply_opacity(Self::premul(color));

        // 使用fontdue Layout引擎排版
        let layout = Self::run_layout(font, text, opts, pos.x, pos.y);
        let glyphs = layout.glyphs();

        for gp in glyphs {
            let (metrics, coverage) = font.font.rasterize_config(gp.key);
            if metrics.width == 0 || metrics.height == 0 { continue; }
            let gx = gp.x as i32;
            let gy = gp.y as i32; // PositiveYDown: glyph top

            for row in 0..metrics.height {
                let sy = gy + row as i32;
                for col in 0..metrics.width {
                    let cov = coverage[row * metrics.width + col];
                    if cov == 0 { continue; }
                    let alpha = (c >> 24) & 0xFF;
                    let cov_u32 = cov as u32;
                    let blended_alpha = (alpha * cov_u32 / 255).min(255);
                    let r = ((c >> 16) & 0xFF) * cov_u32 / 255;
                    let g = ((c >> 8) & 0xFF) * cov_u32 / 255;
                    let b = (c & 0xFF) * cov_u32 / 255;
                    let pixel = (blended_alpha << 24) | (r << 16) | (g << 8) | b;
                    self.put_pixel_raw(gx + col as i32, sy, pixel);
                }
            }
        }

        // 调试辅助线
        if std::env::var("UIX_TTF_DEBUG").as_deref() == Ok("1") && !glyphs.is_empty() {
            let debug_color = 0x80FF0000u32;
            let baseline_color = 0xFF0088FFu32;
            let advance_color = 0xFFFF00FFu32;
            for gp in glyphs {
                let (m, _) = font.font.rasterize_config(gp.key);
                let bx = gp.x as i32;
                let by = gp.y as i32;
                let bw = m.width as i32;
                let bh = m.height as i32;
                for row in 0..bh {
                    let sy = by + row;
                    if row == 0 || row == bh - 1 {
                        for col in 0..bw { self.put_pixel_raw(bx + col, sy, debug_color); }
                    } else {
                        self.put_pixel_raw(bx, sy, debug_color);
                        self.put_pixel_raw(bx + bw - 1, sy, debug_color);
                    }
                }
                let adv_x = gp.x as i32 + m.advance_width as i32;
                for row in 0..bh.min(8) { self.put_pixel_raw(adv_x, by + row, advance_color); }
            }
            // 蓝色基线 — 从 Layout 获取第一行的基准线
            let bl_y = pos.y as i32 + font.font.horizontal_line_metrics(fs)
                .map(|m| m.ascent as i32).unwrap_or((fs * 0.8) as i32);
            for x in (pos.x as i32)..=(pos.x + 200.0) as i32 {
                self.put_pixel_raw(x, bl_y, baseline_color);
                self.put_pixel_raw(x, bl_y + 1, baseline_color);
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
