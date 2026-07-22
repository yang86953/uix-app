//! ab_glyph 后端：纯 advance 定位。字形缓存已统一移到 FontService。

use crate::core::{Errc, Error};
use crate::draw::font::text_backend::{self, *};
use crate::draw::FontHandle;
use crate::draw::TextBackend;
use ab_glyph::*;
use std::sync::Arc;
pub(crate) use text_backend::TextLayoutOptions;
use text_backend::TOFU_GLYPH_ID;

pub(crate) struct FontSlot {
    handle: FontHandle,
    font: FontVec,
}

pub struct AbGlyphBackend {
    pub(crate) fonts: Vec<FontSlot>,
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

fn padded_area_coverage(
    outlined: &OutlinedGlyph,
    width: usize,
    height: usize,
) -> Option<Arc<[u8]>> {
    let bounds = outlined.px_bounds();
    let inner_w = bounds.width() as usize;
    let inner_h = bounds.height() as usize;
    let pad = crate::draw::font::glyph_outline::ATLAS_PAD;
    let expected_w = inner_w.checked_add(pad.checked_mul(2)?)?;
    let expected_h = inner_h.checked_add(pad.checked_mul(2)?)?;
    if width != expected_w || height != expected_h {
        return None;
    }

    let pixel_count = width.checked_mul(height)?;
    let mut coverage = Vec::new();
    if coverage.try_reserve_exact(pixel_count).is_err() {
        return None;
    }
    coverage.resize(pixel_count, 0u8);
    outlined.draw(|x, y, cov| {
        let Some(dst_x) = (x as usize).checked_add(pad) else {
            return;
        };
        let Some(dst_y) = (y as usize).checked_add(pad) else {
            return;
        };
        if dst_x < width && dst_y < height {
            coverage[dst_y * width + dst_x] = (cov * 255.0).clamp(0.0, 255.0) as u8;
        }
    });
    Some(coverage.into())
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
        let fs = text_backend::bounded_font_size(opts.font_size);
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
        let tofu_adv = (fs * 0.55).max(4.0);

        let mut out = Vec::new();
        let mut cx = 0.0f32;
        let mut cy = asc;
        let mut prev = GlyphId(0);
        let mut char_index = 0usize;

        for ch in text.chars() {
            if ch == '\n' {
                cx = 0.0;
                cy += line_h;
                prev = GlyphId(0);
                char_index += 1;
                continue;
            }
            let gid = f.glyph_id(ch);
            let (glyph_id, adv) = if gid == GlyphId(0) {
                // 缺字：保留占位 advance，避免字符消失与索引错位
                (TOFU_GLYPH_ID, tofu_adv)
            } else {
                if prev != GlyphId(0) {
                    cx += sf.kern(prev, gid);
                }
                (gid.0 as u32, sf.h_advance(gid))
            };

            if max_w && cx > 0.0 && cx + adv > opts.max_width {
                cx = 0.0;
                cy += line_h;
            }

            out.push(PositionedGlyph {
                x: cx,
                y: cy,
                width: adv,
                height: font_h,
                glyph_id,
                char_index,
                font: *font,
            });

            cx += adv;
            prev = if glyph_id == TOFU_GLYPH_ID {
                GlyphId(0)
            } else {
                gid
            };
            char_index += 1;
        }

        let text_h = cy - asc + font_h;
        let max_h = if opts.max_height > 0.0 {
            opts.max_height
        } else {
            text_h
        };
        let v_off = if max_h > text_h {
            match opts.v_align {
                crate::draw::VAlign::Top | crate::draw::VAlign::Baseline => 0.0,
                crate::draw::VAlign::Middle => (max_h - text_h) * 0.5,
                crate::draw::VAlign::Bottom => max_h - text_h,
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
                end_char: text.chars().count(),
                glyph_start: 0,
                glyph_count: n_glyphs,
            }],
            width: tw,
            height: max_h.max(text_h),
        }
    }

    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster {
        let Some(pixel_size) = text_backend::normalized_raster_pixel_size(pixel_size) else {
            return GlyphRaster::empty();
        };
        let Some(idx) = self.idx(font) else {
            return GlyphRaster::empty();
        };
        let pixel_size = pixel_size as f32;
        let gid = GlyphId(glyph_id as u16);
        let f = &self.fonts[idx].font;
        let glyph = gid.with_scale_and_position(pixel_size, point(0.0, 0.0));
        let scale_factor = f
            .as_scaled(PxScale {
                x: pixel_size,
                y: pixel_size,
            })
            .scale_factor();
        let glyph_position = glyph.position;
        let outlined = f.outline_glyph(glyph);
        // 同一份原始轮廓同时生成两种数据：字体光栅器的真实面积覆盖率供近
        // 1:1 R8 使用，展平边列表供缩放/仿射 MSDF 使用。
        if let (Some(outline), Some(outlined)) = (f.outline(gid), outlined.as_ref()) {
            let px_bounds = outlined.px_bounds();
            if let Some((w, h, bx, by, mesh)) = crate::draw::font::glyph_outline::mesh_from_outline(
                &outline,
                scale_factor,
                px_bounds,
                glyph_position,
            ) {
                let coverage =
                    padded_area_coverage(outlined, w, h).unwrap_or_else(|| Arc::<[u8]>::from([]));
                return GlyphRaster {
                    width: w,
                    height: h,
                    coverage,
                    bearing_x: bx,
                    bearing_y: by,
                    outline_mesh: Some(mesh),
                };
            }
        }
        // 回退：无轮廓或展平失败时仍走 ab_glyph 面积 coverage。
        let (w, h, data, bx, by) = if let Some(o) = outlined {
            let b = o.px_bounds();
            let bw = (b.max.x - b.min.x).ceil() as usize;
            let bh = (b.max.y - b.min.y).ceil() as usize;
            let bearing_x = b.min.x;
            let bearing_y = b.min.y;
            if let Some(pixel_count) = bw.checked_mul(bh).filter(|count| *count > 0) {
                let mut p = Vec::new();
                if p.try_reserve_exact(pixel_count).is_err() {
                    return GlyphRaster::empty();
                }
                p.resize(pixel_count, 0u8);
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
            coverage: Arc::<[u8]>::from(data),
            bearing_x: bx,
            bearing_y: by,
            outline_mesh: None,
        }
    }

    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics> {
        let i = self.idx(font)?;
        let pixel_size = text_backend::normalized_raster_pixel_size(pixel_size)? as f32;
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
