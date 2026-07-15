//! Ant Design 5 Design Token System — modular architecture.
//!
//! Decomposed into domain-specific sub-traits (`IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, `IBoxShadowTokens`) and aggregated by `TokenProvider`.
//! Concrete presets in `DesignTokens` with light & dark modes.
//!
//! ## Architecture
//!
//! - `IColorTokens` — brand, neutral, semantic, and shadow color tokens
//! - `ITypographyTokens` — font family, sizes, weights, line height
//! - `ISpacingTokens` — padding, border radius, control sizes, motion, breakpoints
//! - `IBoxShadowTokens` — structured multi-layer box shadows
//! - `TokenProvider` — supertrait aggregating all above; add `is_dark()` directly
//! - `DesignTokens` — concrete provider with `antd_light()` & `antd_dark()` presets
//! - `Theme` — wraps `Arc<dyn TokenProvider>` for runtime polymorphism
//! - `PaintContext` carries `&dyn TokenProvider` for widget rendering

pub(crate) mod color_tokens;
pub(crate) mod design_tokens;
mod palette;
pub(crate) mod spacing_tokens;
pub(crate) mod typography_tokens;
pub(crate) mod wrapper;

pub use crate::draw::painting::TokenPatch;
pub use color_tokens::*;
pub use design_tokens::primitives::ThemePrimitives;
pub use design_tokens::*;
pub use palette::*;
// 注：ISpacingTokens / IBoxShadowTokens / ITypographyTokens 已迁移至 ui::traits::theme。
// spacing_tokens 和 typography_tokens 模块保留作为模块结构文档
pub use wrapper::*;
