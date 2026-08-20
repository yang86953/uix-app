//! 字体加载、文本布局与字形渲染。

pub mod backend;
pub mod bitmap_font;
// 公开随应用分发的确定性字体包值契约。
pub mod bundle;
// 保留既有公开模块路径，字体服务实现物理归入 service。
#[path = "service/mod.rs"]
pub mod font_service;
pub(crate) mod glyph_outline;
// 在普通文本与富文本布局之间共享段落级 UAX #9 双向分析。
pub(crate) mod bidi;
// 在字体服务与富文本估算布局之间共享 UAX #14 断行边界。
pub(crate) mod line_break;
// 在编辑控件与文本渲染之间共享显式的字节、字符、字素簇和 shaping cluster 索引契约。
pub mod text;
pub mod text_backend;
// 保留 `font::text_backends` 路径，具体后端归入 text/backends。
#[path = "text/backends/mod.rs"]
pub mod text_backends;
pub mod text_index;

pub use backend::TextBackend;
// 通过 font Module 公开确定性字体配置值。
pub use bundle::FontBundle;
