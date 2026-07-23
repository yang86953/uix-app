//! Color token trait — brand, neutral, semantic, shadow, and link colors.

use crate::draw::Color;

/// 盒阴影令牌（Ant Design 5 多层阴影）。
#[derive(Debug, Clone, Copy, PartialEq)]
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

/// 颜色设计令牌。
pub trait IColorTokens: Send + Sync {
    fn color_primary(&self) -> Color;
    fn color_primary_hover(&self) -> Color;
    fn color_primary_active(&self) -> Color;
    fn color_primary_bg(&self) -> Color;
    fn color_primary_border(&self) -> Color;
    fn color_bg_container(&self) -> Color;
    fn color_bg_elevated(&self) -> Color;
    fn color_bg_raised(&self) -> Color;
    fn color_bg_overlay(&self) -> Color;
    fn color_bg_layout(&self) -> Color;
    fn color_bg_spotlight(&self) -> Color;
    fn color_bg_mask(&self) -> Color;
    fn color_border(&self) -> Color;
    fn color_border_secondary(&self) -> Color;
    fn color_fill(&self) -> Color;
    fn color_fill_secondary(&self) -> Color;
    fn color_fill_tertiary(&self) -> Color;
    fn color_fill_quaternary(&self) -> Color;
    fn color_text(&self) -> Color;
    fn color_text_secondary(&self) -> Color;
    fn color_text_tertiary(&self) -> Color;
    fn color_text_quaternary(&self) -> Color;
    fn color_white(&self) -> Color;
    fn color_black(&self) -> Color;
    fn color_shadow(&self) -> Color;
    fn color_shadow_secondary(&self) -> Color;
    fn color_success(&self) -> Color;
    fn color_success_bg(&self) -> Color;
    fn color_success_border(&self) -> Color;
    fn color_warning(&self) -> Color;
    fn color_warning_bg(&self) -> Color;
    fn color_warning_border(&self) -> Color;
    fn color_error(&self) -> Color;
    fn color_error_bg(&self) -> Color;
    fn color_error_border(&self) -> Color;
    fn color_info(&self) -> Color;
    fn color_info_bg(&self) -> Color;
    fn color_info_border(&self) -> Color;
    fn color_link(&self) -> Color;
    fn color_link_hover(&self) -> Color;
    fn color_link_active(&self) -> Color;
}

/// Neutral semantic roles mapped onto the concrete token fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NeutralRole {
    Text,
    TextSecondary,
    TextTertiary,
    TextQuaternary,
    TextInverse,
    Border,
    BorderSecondary,
    Fill,
    FillSecondary,
    FillTertiary,
    FillQuaternary,
    BgContainer,
    BgElevated,
    BgLayout,
    BgMask,
}
