//! 字体加载、文本布局与字形渲染。

pub mod backend;
pub mod bitmap_font;
pub mod font_service;
pub(crate) mod glyph_outline;
// 在普通文本与富文本布局之间共享段落级 UAX #9 双向分析。
pub(crate) mod bidi;
// 在字体服务与富文本估算布局之间共享 UAX #14 断行边界。
pub(crate) mod line_break;
// 在编辑控件与文本渲染之间共享显式的字节、字符、字素簇和 shaping cluster 索引契约。
pub mod text;
pub mod text_backend;
pub mod text_backends;
pub mod text_index;

pub use backend::TextBackend;
