//! TextBackend trait — abstract font loading, text layout, and glyph rasterization.
//!
//! The `TextBackend` trait decouples the rendering pipeline from any specific
//! font engine.  Each font format family gets its own backend:
//!
//! - `FontdueBackend` — TTF / OTF CFF1 (fontdue crate)
//! - `FreeTypeBackend` — all formats including CFF2 variable fonts (future)
//!
//! The trait is deliberately kept small: load, layout, rasterize, metrics.
//! Higher-level rendering (pixel placement, bitmap fallback) lives in the
//! `SoftwareEngine` / `RenderTarget`.

use crate::diag::Error;
use crate::graphics::{FontHandle, HAlign, VAlign};

/// A single glyph positioned by text layout.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph {
    /// Absolute x position (top-left of glyph bounding box).
    pub x: f32,
    /// Absolute y position (top-left of glyph bounding box).
    pub y: f32,
    /// Width of the glyph's bounding box.
    pub width: f32,
    /// Height of the glyph's bounding box.
    pub height: f32,
    /// Glyph index in the font's internal glyph table.
    pub glyph_id: u32,
}

/// Result of laying out a text string with a `TextBackend`.
#[derive(Debug, Clone)]
pub struct TextLayout {
    /// Sorted positioned glyphs (left-to-right, top-to-bottom).
    pub glyphs: Vec<PositionedGlyph>,
    /// Total width of the laid-out text.
    pub width: f32,
    /// Total height of the laid-out text.
    pub height: f32,
}

/// Rasterized glyph coverage data.
#[derive(Debug, Clone)]
pub struct GlyphRaster {
    /// Width of the coverage bitmap in pixels.
    pub width: usize,
    /// Height of the coverage bitmap in pixels.
    pub height: usize,
    /// Per-pixel α coverage values (0 = transparent, 255 = opaque).
    /// Row-major order, `width * height` elements.
    pub coverage: Vec<u8>,
}

/// Horizontal line metrics for a font at a given pixel size.
#[derive(Debug, Clone, Copy)]
pub struct LineMetrics {
    /// Ascent in pixels (above baseline).
    pub ascent: f32,
    /// Descent in pixels (below baseline, positive value).
    pub descent: f32,
    /// Full line height from baseline to baseline.
    pub new_line_size: f32,
}

/// Text layout options — mirrors the engine-level `TextLayoutOptions` but
/// stays within the text backend layer.
#[derive(Debug, Clone, PartialEq)]
pub struct TextLayoutOptions {
    pub max_width: f32,
    pub max_height: f32,
    pub line_height: f32,
    pub word_wrap: bool,
    pub h_align: HAlign,
    pub v_align: VAlign,
    pub font_size: f32,
}

impl From<crate::graphics::TextLayoutOptions> for TextLayoutOptions {
    fn from(o: crate::graphics::TextLayoutOptions) -> Self {
        Self {
            max_width: o.max_width,
            max_height: o.max_height,
            line_height: o.line_height,
            word_wrap: o.word_wrap,
            h_align: o.h_align,
            v_align: o.v_align,
            font_size: o.font_size,
        }
    }
}

/// Abstract text rendering backend.
///
/// Each implementation owns an internal store of parsed font data.
/// `FontHandle` values (from `load_font`) act as opaque indices into that
/// store.  The backend is fully self-contained — the `SoftwareEngine` simply
/// delegates all text operations through this trait.
pub trait TextBackend: std::fmt::Debug + Send + Sync {
    /// Parse raw font bytes and store for later use.
    ///
    /// Returns an opaque `FontHandle` on success, or an error if the data
    /// cannot be parsed (e.g. unsupported format or corrupt file).
    fn load_font(&mut self, data: &[u8]) -> Result<FontHandle, Error>;

    /// Remove a previously loaded font, releasing its resources.
    fn unload_font(&mut self, handle: &FontHandle);

    /// Check whether a handle still refers to a valid (loaded) font.
    fn is_valid(&self, handle: &FontHandle) -> bool;

    /// Layout a text string, returning positioned glyphs.
    ///
    /// The returned `TextLayout` contains glyphs whose `x`/`y` fields are
    /// absolute positions in the layout coordinate system (origin at top-left).
    fn layout_text(&self, font: &FontHandle, text: &str, opts: &TextLayoutOptions) -> TextLayout;

    /// Rasterize a single glyph at the given pixel size.
    ///
    /// Returns the coverage bitmap (α values per pixel) and dimensions.
    /// The bitmap starts at the top-left of the glyph's bounding box.
    fn rasterize_glyph(&self, font: &FontHandle, glyph_id: u32, pixel_size: f32) -> GlyphRaster;

    /// Horizontal line metrics for a font at `pixel_size`.
    fn horizontal_line_metrics(&self, font: &FontHandle, pixel_size: f32) -> Option<LineMetrics>;
}
