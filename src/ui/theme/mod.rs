//! Ant Design 5 设计令牌系统——模块化架构。
//!
//! 拆分为领域专属子 trait（`IColorTokens`、`ITypographyTokens`、
//! `ISpacingTokens`、`IBoxShadowTokens`），并由 `TokenProvider` 聚合。
//! 具体预设位于 `DesignTokens`，含亮色与暗色两套模式。
//!
//! ## 架构
//!
//! - `IColorTokens` —— 品牌、中性、语义与阴影颜色令牌
//! - `ITypographyTokens` —— 字体族、字号、字重、行高
//! - `ISpacingTokens` —— 内边距、圆角、控件尺寸、动效、断点
//! - `IBoxShadowTokens` —— 结构化多层盒阴影
//! - `TokenProvider` —— 聚合以上全部的超 trait；`is_dark()` 直接加在此处
//! - `DesignTokens` —— 提供 `antd_light()` 与 `antd_dark()` 预设的具体实现
//! - `Theme` —— 包装 `Arc<dyn TokenProvider>` 实现运行时多态
//! - `PaintContext` —— 携带 `&dyn TokenProvider` 供 widget 渲染使用

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
