//! 文本后端实现集合。
//!
//! 实现 `TextBackend` trait 的具体后端，供 `FontService` 使用。
//! 当前仅提供 `ab_glyph` 后端。

pub mod ab_glyph;
// 将 OpenType shaping 实现隔离在字体后端内部，不扩散第三方类型。
mod shaping;
