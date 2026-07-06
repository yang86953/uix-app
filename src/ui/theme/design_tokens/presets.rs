//! Preset theme definitions for DesignTokens.
//!
//! Provides `antd_light()` and `antd_dark()` factory functions that return
//! fully-populated `DesignTokens` instances by deriving from ThemePrimitives.

use super::primitives::ThemePrimitives;
use super::DesignTokens;
use crate::draw::Color;

pub const PRIMARY_COUNT: usize = 12;
pub const PRIMARY_BLUE_INDEX: usize = 8;

impl DesignTokens {
    /// Ant Design 5 亮色主题预设。
    pub fn antd_light() -> Self {
        ThemePrimitives::antd_light().into_design_tokens(false)
    }

    /// Ant Design 5 暗色主题预设。
    pub fn antd_dark() -> Self {
        ThemePrimitives::antd_dark().into_design_tokens(true)
    }

    /// 从自定义基色生成主题。
    pub fn from_primitives(primitives: ThemePrimitives, is_dark: bool) -> Self {
        primitives.into_design_tokens(is_dark)
    }

    /// Replace the default brand primary seed while preserving the mode.
    pub fn with_brand_primary(self, primary: Color) -> Self {
        let mut primitives = if self.is_dark {
            ThemePrimitives::antd_dark()
        } else {
            ThemePrimitives::antd_light()
        };
        primitives.primary = primary;
        primitives.info = primary;
        primitives.into_design_tokens(self.is_dark)
    }

    /// Build a theme from the 12 Ant Design chromatic primary seeds.
    ///
    /// The current token surface has one active brand primary; it uses the Blue
    /// seed from the 12-color array and keeps the rest available to the caller.
    pub fn from_primaries(primaries: [Color; PRIMARY_COUNT], is_dark: bool) -> Self {
        let mut primitives = if is_dark {
            ThemePrimitives::antd_dark()
        } else {
            ThemePrimitives::antd_light()
        };
        let primary = primaries[PRIMARY_BLUE_INDEX];
        primitives.primary = primary;
        primitives.info = primary;
        primitives.into_design_tokens(is_dark)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_brand_primary_replaces_primary_seed() {
        let brand = Color::from_rgb(120, 42, 210);
        let tokens = DesignTokens::antd_light().with_brand_primary(brand);

        assert_eq!(tokens.color_primary, brand);
        assert_eq!(tokens.color_info, brand);
        assert!(!tokens.is_dark);
    }

    #[test]
    fn from_primaries_uses_blue_seed_slot() {
        let mut primaries = [Color::from_rgb(1, 2, 3); PRIMARY_COUNT];
        let blue = Color::from_rgb(22, 119, 255);
        primaries[PRIMARY_BLUE_INDEX] = blue;

        let tokens = DesignTokens::from_primaries(primaries, true);

        assert_eq!(tokens.color_primary, blue);
        assert_eq!(tokens.color_info, blue);
        assert!(tokens.is_dark);
    }
}
