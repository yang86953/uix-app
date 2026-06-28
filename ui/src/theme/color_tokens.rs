//! Color token trait — brand, neutral, semantic, shadow, and link colors.
//!
//! Domain sub-trait of the Ant Design 5 token system. Combined via
//! `TokenProvider` supertrait in `theme.rs`.

use uix_graphics::Color;

// ════════════════════════════════════════════════════════════════════════════
// ShadowToken
// ════════════════════════════════════════════════════════════════════════════

/// Box shadow token (layered shadows as in Ant Design 5).
#[derive(Debug, Clone, Copy)]
pub struct ShadowToken {
    pub layer_1: (f32, f32, f32, Color),
    pub layer_2: (f32, f32, f32, Color),
    pub layer_3: (f32, f32, f32, Color),
}

impl ShadowToken {
    pub const fn none() -> Self {
        Self {
            layer_1: (0.0, 0.0, 0.0, Color::transparent()),
            layer_2: (0.0, 0.0, 0.0, Color::transparent()),
            layer_3: (0.0, 0.0, 0.0, Color::transparent()),
        }
    }
}

// IColorTokens trait 定义已迁移至 api/traits.rs
