//! 绘制上下文与主题快照。

pub mod canvas;
pub mod paint_context;
pub mod theme;
mod token_patch;

pub use canvas::Canvas2D;
pub use paint_context::{resolve_font_size, PaintContext, PaintSurfaceConfig};
pub use theme::{
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ShadowToken, ThemeSnapshot,
    ThemeTokens,
};
pub use token_patch::TokenPatch;
pub(crate) use token_patch::{ScopedThemeTokens, TokenScope};
