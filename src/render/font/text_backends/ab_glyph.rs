//! ab_glyph 后端：纯 advance 定位。字形缓存已统一移到 FontService。

use crate::api::render::text::TextBackend;
use crate::render::text_backend::*;
use crate::api::render::FontHandle;
use ab_glyph::*;
use std::sync::Arc;
use crate::platform::{Errc, Error};

struct FontSlot {
    handle: FontHandle,
    font: FontVec,
}

pub struct AbGlyphBackend {
    fonts: Vec<FontSlot>,
}

impl std::fmt::Debug for AbGlyphBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AbGlyphBackend")
            .field("fonts", &self.fonts.len())
            .finish()
    }
}

impl Default for AbGlyphBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl AbGlyphBackend {
    pub fn new() -> Self {
        Self { fonts: vec![] }
    }
    fn idx(&self, h: &FontHandle) -> Option<usize> {
        let i = h.0 as usize;
        if i < self.fonts.len() {
            Some(i)
        } else {
            None
        }
    }
}

impl TextBackend for AbGlyphBackend {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let f = FontVec::try_from_vec(data.to_vec())
            .map_err(|e| Error::new(Errc::FormatError, format!("ab_glyph: {:?}", e)))?;
        let id = self.fonts.len() as u32;
        self.fonts.push(FontSlot {
            handle: FontHandle::new(id),
            font: f,
        });
        Ok(FontHandle::new(id))
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        let i = handle.0 as usize;
        if i < self.fonts.len() {
            self.fonts[i].handle = FontHandle::new(u32::MAX);
        }
    }

    fn is_valid(&self, h: &FontHandle) -> bool {
        self.idx(h)
            .is_some_and(|i| self.fonts[i].handle.0 != u32::MAX)
    }

    fn has_glyph(&self, font: &FontHandle, ch: char) -> bool {
        self.idx(font)
            .is_some_and(|i| self.fonts[i].font.glyph_id(ch) != GlyphId(0))
    }

    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout {
        let Some(idx) = self.idx(font) else {
            return TextLayout {
                glyphs: vec![],
                lines: vec![],
                width: 0.0,
                height: 0.0,
            };
        };
        let f = &self.fonts[idx].font;
        let fs = opts.font_size.max(1.0);
        let sf = f.as_scaled(PxScale { x: fs, y: fs });

        let asc = sf.ascent();
        let desc = sf.descent();
        let lg = sf.line_gap();
        let font_h = (asc - desc + lg).max(fs);
        let line_h = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            font_h
        };
        let max_w = opts.max_width.is_finite() && opts.max_width > 0.0;

        let mut out = Vec::new();
        let mut cx = 0.0f32;
        let mut cy = asc;
        let mut prev = GlyphId(0);

        for ch in text.chars() {
            if ch == '\n' {
                cx = 0.0;
                cy += line_h;
                prev = GlyphId(0);
                continue;
            }
            let gid = f.glyph_id(ch);
            if gid == GlyphId(0) {
                continue;
            }
            if prev != GlyphId(0) {
                cx += sf.kern(prev, gid);
            }
            let adv = sf.h_advance(gid);

            if max_w && cx > 0.0 && cx + adv > opts.max_width {
                cx = 0.0;
                cy += line_h;
            }

            let glyph_top = cy;
            out.push(PositionedGlyph {
                x: cx,
                y: glyph_top,
                width: adv,
                height: font_h,
                glyph_id: gid.0 as u32,
                font: *font,
            });

            cx += adv;
            prev = gid;
        }

        let text_h = cy - asc + font_h;
        let max_h = if opts.max_height > 0.0 {
            opts.max_height
        } else {
            text_h
        };
        let v_off = if max_h > text_h {
            match opts.v_align {
                crate::api::render::VAlign::Top | crate::api::render::VAlign::Baseline => 0.0,
                crate::api::render::VAlign::Middle => (max_h - text_h) * 0.5,
                crate::api::render::VAlign::Bottom => max_h - text_h,
            }
        } else {
            0.0
        };

        for g in out.iter_mut() {
            g.y += v_off;
        }

        let tw = out.iter().fold(0.0f32, |m, g| (g.x + g.width).max(m));
        let n_glyphs = out.len();
        TextLayout {
            glyphs: out,
            lines: vec![LineInfo {
                y: 0.0,
                height: max_h.max(text_h),
                width: tw,
                start_char: 0,
                end_char: text.len(),
                glyph_start: 0,
                glyph_count: n_glyphs,
            }],
            width: tw,
            height: max_h.max(text_h),
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
        let sc = self.fonts[i].font.as_scaled(PxScale {
            x: pixel_size,
            y: pixel_size,
        });
        Some(LineMetrics {
            ascent: sc.ascent(),
            descent: -sc.descent(),
            new_line_size: sc.height(),
        })
    }

    fn clear_cache(&mut self) { /* 缓存已统一在 FontService 层 */
    }

    fn memory_usage(&self) -> usize {
        let mut t = 0usize;
        for s in &self.fonts {
            if s.handle.0 != u32::MAX {
                t += s.font.as_slice().len();
            }
        }
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_creates_empty_backend() {
        let backend = AbGlyphBackend::new();
        assert!(backend.fonts.is_empty());
    }

    #[test]
    fn load_font_empty_data_returns_format_error() {
        let mut backend = AbGlyphBackend::new();
        let result = backend.load_font(&[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.code(), crate::platform::Errc::FormatError);
    }

    #[test]
    fn unload_font_invalid_handle_does_not_panic() {
        let mut backend = AbGlyphBackend::new();
        backend.unload_font(&FontHandle::new(999));
        // 没有 panic 即通过
    }

    #[test]
    fn is_valid_unloaded_handle_returns_false() {
        let backend = AbGlyphBackend::new();
        assert!(!backend.is_valid(&FontHandle::new(0)));
    }

    #[test]
    fn has_glyph_unloaded_handle_returns_false() {
        let backend = AbGlyphBackend::new();
        assert!(!backend.has_glyph(&FontHandle::new(0), 'a'));
    }

    #[test]
    fn layout_text_unloaded_handle_returns_empty_layout() {
        let backend = AbGlyphBackend::new();
        let opts = super::TextLayoutOptions {
            max_width: f32::MAX,
            max_height: 0.0,
            line_height: 0.0,
            word_wrap: true,
            h_align: crate::api::render::HAlign::Left,
            v_align: crate::api::render::VAlign::Top,
            font_size: 14.0,
        };
        let layout = backend.layout_text(&FontHandle::new(0), "hello", &opts);
        assert_eq!(layout.width, 0.0);
        assert_eq!(layout.height, 0.0);
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn layout_text_empty_string_unloaded_handle_returns_empty_layout() {
        let backend = AbGlyphBackend::new();
        let opts = super::TextLayoutOptions {
            max_width: f32::MAX,
            max_height: 0.0,
            line_height: 0.0,
            word_wrap: true,
            h_align: crate::api::render::HAlign::Left,
            v_align: crate::api::render::VAlign::Top,
            font_size: 14.0,
        };
        let layout = backend.layout_text(&FontHandle::new(0), "", &opts);
        assert_eq!(layout.width, 0.0);
        assert_eq!(layout.height, 0.0);
        assert!(layout.glyphs.is_empty());
    }

    #[test]
    fn rasterize_glyph_invalid_handle_returns_empty_raster() {
        let backend = AbGlyphBackend::new();
        let raster = backend.rasterize_glyph(&FontHandle::new(0), 0, 14.0);
        assert_eq!(raster.width, 0);
        assert_eq!(raster.height, 0);
        assert!(raster.coverage.is_empty());
    }

    #[test]
    fn horizontal_line_metrics_invalid_handle_returns_none() {
        let backend = AbGlyphBackend::new();
        let result = backend.horizontal_line_metrics(&FontHandle::new(0), 14.0);
        assert!(result.is_none());
    }

    #[test]
    fn memory_usage_empty_backend_returns_zero() {
        let backend = AbGlyphBackend::new();
        assert_eq!(backend.memory_usage(), 0);
    }

    #[test]
    fn clear_cache_does_not_panic() {
        let mut backend = AbGlyphBackend::new();
        backend.clear_cache();
        // 没有 panic 即通过
    }

    #[test]
    fn font_data_default_returns_none() {
        // font_data 的默认实现返回 None
        let backend = AbGlyphBackend::new();
        let result = TextBackend::font_data(&backend, &FontHandle::new(0));
        assert!(result.is_none());
    }

    #[test]
    fn set_fallback_fonts_does_not_panic() {
        let mut backend = AbGlyphBackend::new();
        TextBackend::set_fallback_fonts(&mut backend, &[]);
        // 没有 panic 即通过
    }

    #[test]
    fn debug_format_does_not_panic() {
        let backend = AbGlyphBackend::new();
        let _ = format!("{:?}", backend);
    }
}
