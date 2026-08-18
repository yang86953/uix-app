//! 字体与位图资源服务。

pub mod font;
pub mod image;

pub use font::bitmap_font::BitmapFont;
// 公开跨平台确定性字体包配置。
pub use font::bundle::FontBundle;
pub use font::font_service::FontService;
pub use font::text_backend::{GlyphRaster, LineInfo, LineMetrics, PositionedGlyph, TextLayout};
pub use image::{BitmapHandle, ImageService, ImageSlot};
