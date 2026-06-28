//! ab_glyph 后端：纯 advance 定位。字形缓存已统一移到 FontService。

use std::sync::Arc;
use ab_glyph::*;
use uix_platform::{Errc, Error};
use crate::text_backend::*;
use crate::FontHandle;

struct FontSlot { handle: FontHandle, font: FontVec }

pub struct AbGlyphBackend {
    fonts: Vec<FontSlot>,
}

impl std::fmt::Debug for AbGlyphBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbGlyphBackend").field("fonts", &self.fonts.len()).finish()
    }
}

impl Default for AbGlyphBackend { fn default() -> Self { Self::new() } }

impl AbGlyphBackend {
    pub fn new() -> Self {
        Self { fonts: vec![] }
    }
    fn idx(&self, h: &FontHandle) -> Option<usize> {
        let i = h.0 as usize;
        if i < self.fonts.len() { Some(i) } else { None }
    }
}

impl TextBackend for AbGlyphBackend {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let f = FontVec::try_from_vec(data.to_vec())
            .map_err(|e| Error::new(Errc::FormatError, format!("ab_glyph: {:?}", e)))?;
        let id = self.fonts.len() as u32;
        self.fonts.push(FontSlot { handle: FontHandle::new(id), font: f });
        Ok(FontHandle::new(id))
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        let i = handle.0 as usize;
        if i < self.fonts.len() {
            self.fonts[i].handle = FontHandle::new(u32::MAX);
        }
    }

    fn is_valid(&self, h: &FontHandle) -> bool {
        self.idx(h).map_or(false, |i| self.fonts[i].handle.0 != u32::MAX)
    }

    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.idx(font).map_or(false, |i| self.fonts[i].font.glyph_id(ch) != GlyphId(0))
    }

    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout {
        let Some(idx) = self.idx(font) else {
            return TextLayout { glyphs: vec![], lines: vec![], width: 0.0, height: 0.0 };
        };
        let f = &self.fonts[idx].font;
        let fs = opts.font_size.max(1.0);
        let sf = f.as_scaled(PxScale { x: fs, y: fs });

        let asc = sf.ascent();
        let desc = sf.descent();
        let lg = sf.line_gap();
        let font_h = (asc - desc + lg).max(fs);
        let line_h = if opts.line_height > 0.0 { opts.line_height } else { font_h };
        let max_w = opts.max_width.is_finite() && opts.max_width > 0.0;

        let mut out = Vec::new();
        let mut cx = 0.0f32;
        let mut cy = asc;
        let mut prev = GlyphId(0);

        for ch in text.chars() {
            if ch == '\n' { cx = 0.0; cy += line_h; prev = GlyphId(0); continue; }
            let gid = f.glyph_id(ch);
            if gid == GlyphId(0) { continue; }
            if prev != GlyphId(0) { cx += sf.kern(prev, gid); }
            let adv = sf.h_advance(gid);

            if max_w && cx > 0.0 && cx + adv > opts.max_width {
                cx = 0.0; cy += line_h;
            }

            let glyph_top = cy;
            out.push(PositionedGlyph { x: cx, y: glyph_top, width: adv, height: font_h, glyph_id: gid.0 as u32, font: *font });

            cx += adv;
            prev = gid;
        }

        let text_h = cy - asc + font_h;
        let max_h = if opts.max_height > 0.0 { opts.max_height } else { text_h };
        let v_off = if max_h > text_h {
            match opts.v_align {
                crate::VAlign::Top | crate::VAlign::Baseline => 0.0,
                crate::VAlign::Middle => (max_h - text_h) * 0.5,
                crate::VAlign::Bottom => max_h - text_h,
            }
        } else { 0.0 };

        for g in out.iter_mut() { g.y += v_off; }

        let tw = out.iter().fold(0.0f32, |m, g| (g.x + g.width).max(m));
        let n_glyphs = out.len();
        TextLayout {
            glyphs: out,
            lines: vec![LineInfo { y: 0.0, height: max_h.max(text_h), width: tw,
                start_char: 0, end_char: text.len(), glyph_start: 0, glyph_count: n_glyphs }],
            width: tw, height: max_h.max(text_h),
        }
    }

    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster {
        let Some(idx) = self.idx(font) else {
            return GlyphRaster {
                width: 0,
                height: 0,
                coverage: Arc::new(vec![]),
                bearing_x: 0.0,
                bearing_y: 0.0,
            };
        };
        let gid = GlyphId(glyph_id as u16);
        let f = &self.fonts[idx].font;
        let glyph = gid.with_scale_and_position(pixel_size, point(0.0, 0.0));
        let (w, h, data, bx, by) = if let Some(o) = f.outline_glyph(glyph) {
            let b = o.px_bounds();
            let bw = (b.max.x - b.min.x).ceil() as usize;
            let bh = (b.max.y - b.min.y).ceil() as usize;
            let bearing_x = b.min.x;
            let bearing_y = b.min.y;
            if bw > 0 && bh > 0 {
                let mut p = vec![0u8; bw * bh];
                o.draw(|x, y, cov| {
                    if (x as usize) < bw && (y as usize) < bh {
                        p[y as usize * bw + x as usize] = (cov * 255.0) as u8;
                    }
                });
                (bw, bh, p, bearing_x, bearing_y)
            } else {
                (0, 0, vec![], 0.0, 0.0)
            }
        } else {
            (0, 0, vec![], 0.0, 0.0)
        };

        GlyphRaster {
            width: w,
            height: h,
            coverage: Arc::new(data),
            bearing_x: bx,
            bearing_y: by,
        }
    }

    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics> {
        let i = self.idx(font)?;
        let sc = self.fonts[i].font.as_scaled(PxScale { x: pixel_size, y: pixel_size });
        Some(LineMetrics { ascent: sc.ascent(), descent: -sc.descent(), new_line_size: sc.height() })
    }

    fn clear_cache(&mut self) { /* 缓存已统一在 FontService 层 */ }

    fn memory_usage(&self) -> usize {
        let mut t = 0usize;
        for s in &self.fonts { if s.handle.0 != u32::MAX { t += s.font.as_slice().len(); } }
        t
    }
}
