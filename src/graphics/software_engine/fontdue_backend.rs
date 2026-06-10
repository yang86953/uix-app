//! FontdueBackend — `TextBackend` implementation using the `fontdue` crate.
//!
//! Supports TrueType outlines (`glyf`/`loca`) and CFF1 PostScript outlines.
//! Does **not** support CFF2 variable fonts (`.ttc` with OTTO sfVersion)
//! because fontdue 0.9 does not process the `gvar`/`CFF2` variation tables.
//! Use a `FreeTypeBackend` (future) for complete format coverage.

use crate::diag::{Errc, Error};
use crate::graphics::text_backend::{
    GlyphRaster, LineMetrics, PositionedGlyph, TextBackend, TextLayout, TextLayoutOptions,
};
use crate::graphics::FontHandle;
use fontdue::layout::*;

/// Internal slot for a loaded font.
#[derive(Debug)]
struct FontSlot {
    handle: FontHandle,
    font: fontdue::Font,
}

/// Fontdue-based text backend.
///
/// Stores parsed fonts in an internal `Vec`. Each `FontHandle` is an index
/// into this vector. Thread-safe after construction (all methods take `&self`
/// except `load_font`/`unload_font` which require `&mut self`).
#[derive(Debug, Default)]
pub struct FontdueBackend {
    fonts: Vec<FontSlot>,
}

impl FontdueBackend {
    pub fn new() -> Self {
        Self { fonts: Vec::new() }
    }

    /// Run the fontdue Layout engine, returning positioned glyphs that share
    /// the same coordinate system (top-left origin, positive Y down).
    fn run_layout(
        font: &fontdue::Font,
        text: &str,
        opts: &TextLayoutOptions,
        pos_x: f32,
        pos_y: f32,
    ) -> Layout<()> {
        let fs = opts.font_size.max(1.0);
        let new_line_size = font
            .horizontal_line_metrics(fs)
            .map(|m| m.new_line_size)
            .unwrap_or(fs * 1.3);
        let lh_abs = if opts.line_height > 0.0 {
            opts.line_height
        } else {
            new_line_size
        };
        let lh_mult = lh_abs / new_line_size;
        let max_w = if opts.max_width.is_finite() && opts.max_width > 0.0 {
            Some(opts.max_width)
        } else {
            None
        };
        let max_h = if opts.max_height > 0.0 {
            Some(opts.max_height)
        } else {
            None
        };

        let h_align = match opts.h_align {
            crate::graphics::HAlign::Left => HorizontalAlign::Left,
            crate::graphics::HAlign::Center => HorizontalAlign::Center,
            crate::graphics::HAlign::Right => HorizontalAlign::Right,
        };
        let v_align = match opts.v_align {
            crate::graphics::VAlign::Top => VerticalAlign::Top,
            crate::graphics::VAlign::Middle => VerticalAlign::Middle,
            crate::graphics::VAlign::Bottom => VerticalAlign::Bottom,
            crate::graphics::VAlign::Baseline => VerticalAlign::Top,
        };
        let wrap = if opts.word_wrap {
            WrapStyle::Word
        } else {
            WrapStyle::Letter
        };

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: pos_x,
            y: pos_y,
            max_width: max_w,
            max_height: max_h,
            horizontal_align: h_align,
            vertical_align: v_align,
            line_height: lh_mult,
            wrap_style: wrap,
            wrap_hard_breaks: true,
        });
        layout.append(&[font], &TextStyle::new(text, fs, 0));
        layout
    }
}

impl TextBackend for FontdueBackend {
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error> {
        let font = fontdue::Font::from_bytes(data, fontdue::FontSettings::default())
            .map_err(|e| Error::new(Errc::FormatError, format!("fontdue parse failed: {}", e)))?;
        let idx = self.fonts.len() as u32;
        self.fonts.push(FontSlot {
            handle: FontHandle::new(idx),
            font,
        });
        Ok(FontHandle::new(idx))
    }

    fn unload_font(&mut self, handle: &FontHandle) {
        let idx = handle.0 as usize;
        if idx < self.fonts.len() {
            // Replace with a placeholder to keep indices stable.
            // This prevents handle reuse from silently pointing to a different font.
            self.fonts[idx] = FontSlot {
                handle: FontHandle::new(u32::MAX),
                font: fontdue::Font::from_bytes(
                    &[] as &[u8],
                    fontdue::FontSettings::default(),
                )
                .unwrap_or_else(|_| {
                    // Minimal empty font — should never fail because empty data
                    // is not valid, but we need the slot to exist.  In practice
                    // a successfully-loaded font will only be unloaded during
                    // engine shutdown, so this code path is rarely hit.
                    panic!("fontdue_backend: failed to create placeholder font")
                }),
            };
        }
    }

    fn is_valid(&self, handle: &FontHandle) -> bool {
        let idx = handle.0 as usize;
        idx < self.fonts.len() && self.fonts[idx].handle.0 != u32::MAX
    }

    fn layout_text(
        &self,
        font: &FontHandle,
        text: &str,
        opts: &TextLayoutOptions,
    ) -> TextLayout {
        let idx = font.0 as usize;
        let data = match self.fonts.get(idx) {
            Some(d) => d,
            None => {
                return TextLayout {
                    glyphs: Vec::new(),
                    width: 0.0,
                    height: 0.0,
                };
            }
        };

        let layout = Self::run_layout(&data.font, text, opts, 0.0, 0.0);
        let gp = layout.glyphs();
        let glyphs: Vec<PositionedGlyph> = gp
            .iter()
            .map(|g| PositionedGlyph {
                x: g.x,
                y: g.y,
                width: g.width as f32,
                height: g.height as f32,
                glyph_id: u32::from(g.key.glyph_index),
            })
            .collect();

        let max_x = glyphs
            .iter()
            .fold(0.0f32, |m, g| (g.x + g.width.max(0.0)).max(m));

        TextLayout {
            width: max_x,
            height: opts.font_size.max(0.0),
            glyphs,
        }
    }

    fn rasterize_glyph(
        &self,
        font: &FontHandle,
        glyph_id: u32,
        pixel_size: f32,
    ) -> GlyphRaster {
        let idx = font.0 as usize;
        let data = match self.fonts.get(idx) {
            Some(d) => d,
            None => {
                return GlyphRaster {
                    width: 0,
                    height: 0,
                    coverage: Vec::new(),
                };
            }
        };

        let glyph_index = (glyph_id as u16).min(data.font.glyph_count() - 1);
        let (metrics, coverage) = data.font.rasterize_indexed(glyph_index, pixel_size);
        GlyphRaster {
            width: metrics.width,
            height: metrics.height,
            coverage,
        }
    }

    fn horizontal_line_metrics(
        &self,
        font: &FontHandle,
        pixel_size: f32,
    ) -> Option<LineMetrics> {
        let idx = font.0 as usize;
        let data = self.fonts.get(idx)?;
        data.font
            .horizontal_line_metrics(pixel_size)
            .map(|m| LineMetrics {
                ascent: m.ascent,
                descent: m.descent,
                new_line_size: m.new_line_size,
            })
    }
}
