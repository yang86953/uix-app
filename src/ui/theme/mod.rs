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
mod token_patch;
pub(crate) mod typography_tokens;
pub(crate) mod wrapper;

pub mod style;
pub(crate) mod traits;
pub use color_tokens::*;
pub use design_tokens::primitives::ThemePrimitives;
pub use design_tokens::*;
pub use palette::*;
pub use token_patch::TokenPatch;
pub(crate) use token_patch::{ScopedThemeTokens, TokenScope};
pub use wrapper::*;
