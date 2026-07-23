//! 字体与位图资源服务。

pub mod font;
pub mod image;

pub use font::bitmap_font::BitmapFont;
pub use font::font_service::FontService;
pub use font::text_backend::{GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, TextLayout};
pub use image::{BitmapHandle, ImageService, ImageSlot};
