//! DesignTokens — concrete Ant Design 5 token preset with light & dark modes.
//!
//! Implements all token sub-traits (`IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, `IBoxShadowTokens`, `TokenProvider`) by delegating to
//! public fields. Factory functions `antd_light()` and `antd_dark()` provide
//! complete Ant Design 5 presets (defined in the `presets` submodule).

pub mod presets;
pub mod primitives;

use super::color_tokens::ShadowToken;
use crate::draw::painting::{
    IBoxShadowTokens, IColorTokens, ISpacingTokens, ITypographyTokens, ThemeTokens,
};
use crate::ui::traits::TokenProvider;
use crate::draw::Color;

#[derive(Debug, Clone)]
pub struct DesignTokens {
    pub color_primary: Color,
    pub color_primary_hover: Color,
    pub color_primary_active: Color,
    pub color_primary_bg: Color,
    pub color_primary_border: Color,
    pub color_bg_container: Color,
    pub color_bg_elevated: Color,
    pub color_bg_raised: Color,
    pub color_bg_overlay: Color,
    pub color_bg_layout: Color,
    pub color_bg_spotlight: Color,
    pub color_bg_mask: Color,
    pub color_border: Color,
    pub color_border_secondary: Color,
    pub color_fill: Color,
    pub color_fill_secondary: Color,
    pub color_fill_tertiary: Color,
    pub color_fill_quaternary: Color,
    pub color_text: Color,
    pub color_text_secondary: Color,
    pub color_text_tertiary: Color,
    pub color_text_quaternary: Color,
    pub color_white: Color,
    pub color_black: Color,
    pub color_shadow: Color,
    pub color_shadow_secondary: Color,
    pub color_success: Color,
    pub color_success_bg: Color,
    pub color_success_border: Color,
    pub color_warning: Color,
    pub color_warning_bg: Color,
    pub color_warning_border: Color,
    pub color_error: Color,
    pub color_error_bg: Color,
    pub color_error_border: Color,
    pub color_info: Color,
    pub color_info_bg: Color,
    pub color_info_border: Color,
    pub color_link: Color,
    pub color_link_hover: Color,
    pub color_link_active: Color,
    pub font_family: &'static str,
    pub font_size_sm: f32,
    pub font_size: f32,
    pub font_size_lg: f32,
    pub font_size_xl: f32,
    pub font_size_heading_1: f32,
    pub font_size_heading_2: f32,
    pub font_size_heading_3: f32,
    pub font_size_heading_4: f32,
    pub font_size_heading_5: f32,
    pub font_weight_regular: f32,
    pub font_weight_medium: f32,
    pub font_weight_semibold: f32,
    pub font_weight_bold: f32,
    pub line_height: f32,
    pub padding_xss: f32,
    pub padding_xs: f32,
    pub padding_sm: f32,
    pub padding: f32,
    pub padding_md: f32,
    pub padding_lg: f32,
    pub padding_xl: f32,
    pub border_radius: f32,
    pub border_radius_sm: f32,
    pub border_radius_lg: f32,
    pub border_radius_xl: f32,
    pub border_radius_round: f32,
    pub control_height_sm: f32,
    pub control_height: f32,
    pub control_height_lg: f32,
    pub box_shadow: ShadowToken,
    pub box_shadow_secondary: ShadowToken,
    pub motion_duration_fast: f32,
    pub motion_duration_mid: f32,
    pub motion_duration_slow: f32,
    pub motion_easing_default: &'static str,
    pub motion_easing_in: &'static str,
    pub motion_easing_out: &'static str,
    pub motion_easing_in_out: &'static str,
    pub screen_xs: f32,
    pub screen_sm: f32,
    pub screen_md: f32,
    pub screen_lg: f32,
    pub screen_xl: f32,
    pub screen_xxl: f32,
    pub is_dark: bool,
}

// ── Trait implementations ──

impl IColorTokens for DesignTokens {
    fn color_primary(&self) -> Color {
        self.color_primary
    }
    fn color_primary_hover(&self) -> Color {
        self.color_primary_hover
    }
    fn color_primary_active(&self) -> Color {
        self.color_primary_active
    }
    fn color_primary_bg(&self) -> Color {
        self.color_primary_bg
    }
    fn color_primary_border(&self) -> Color {
        self.color_primary_border
    }
    fn color_bg_container(&self) -> Color {
        self.color_bg_container
    }
    fn color_bg_elevated(&self) -> Color {
        self.color_bg_elevated
    }
    fn color_bg_raised(&self) -> Color {
        self.color_bg_raised
    }
    fn color_bg_overlay(&self) -> Color {
        self.color_bg_overlay
    }
    fn color_bg_layout(&self) -> Color {
        self.color_bg_layout
    }
    fn color_bg_spotlight(&self) -> Color {
        self.color_bg_spotlight
    }
    fn color_bg_mask(&self) -> Color {
        self.color_bg_mask
    }
    fn color_border(&self) -> Color {
        self.color_border
    }
    fn color_border_secondary(&self) -> Color {
        self.color_border_secondary
    }
    fn color_fill(&self) -> Color {
        self.color_fill
    }
    fn color_fill_secondary(&self) -> Color {
        self.color_fill_secondary
    }
    fn color_fill_tertiary(&self) -> Color {
        self.color_fill_tertiary
    }
    fn color_fill_quaternary(&self) -> Color {
        self.color_fill_quaternary
    }
    fn color_text(&self) -> Color {
        self.color_text
    }
    fn color_text_secondary(&self) -> Color {
        self.color_text_secondary
    }
    fn color_text_tertiary(&self) -> Color {
        self.color_text_tertiary
    }
    fn color_text_quaternary(&self) -> Color {
        self.color_text_quaternary
    }
    fn color_white(&self) -> Color {
        self.color_white
    }
    fn color_black(&self) -> Color {
        self.color_black
    }
    fn color_shadow(&self) -> Color {
        self.color_shadow
    }
    fn color_shadow_secondary(&self) -> Color {
        self.color_shadow_secondary
    }
    fn color_success(&self) -> Color {
        self.color_success
    }
    fn color_success_bg(&self) -> Color {
        self.color_success_bg
    }
    fn color_success_border(&self) -> Color {
        self.color_success_border
    }
    fn color_warning(&self) -> Color {
        self.color_warning
    }
    fn color_warning_bg(&self) -> Color {
        self.color_warning_bg
    }
    fn color_warning_border(&self) -> Color {
        self.color_warning_border
    }
    fn color_error(&self) -> Color {
        self.color_error
    }
    fn color_error_bg(&self) -> Color {
        self.color_error_bg
    }
    fn color_error_border(&self) -> Color {
        self.color_error_border
    }
    fn color_info(&self) -> Color {
        self.color_info
    }
    fn color_info_bg(&self) -> Color {
        self.color_info_bg
    }
    fn color_info_border(&self) -> Color {
        self.color_info_border
    }
    fn color_link(&self) -> Color {
        self.color_link
    }
    fn color_link_hover(&self) -> Color {
        self.color_link_hover
    }
    fn color_link_active(&self) -> Color {
        self.color_link_active
    }
}

impl ITypographyTokens for DesignTokens {
    fn font_family(&self) -> &str {
        self.font_family
    }
    fn font_size_sm(&self) -> f32 {
        self.font_size_sm
    }
    fn font_size(&self) -> f32 {
        self.font_size
    }
    fn font_size_lg(&self) -> f32 {
        self.font_size_lg
    }
    fn font_size_xl(&self) -> f32 {
        self.font_size_xl
    }
    fn font_size_heading_1(&self) -> f32 {
        self.font_size_heading_1
    }
    fn font_size_heading_2(&self) -> f32 {
        self.font_size_heading_2
    }
    fn font_size_heading_3(&self) -> f32 {
        self.font_size_heading_3
    }
    fn font_size_heading_4(&self) -> f32 {
        self.font_size_heading_4
    }
    fn font_size_heading_5(&self) -> f32 {
        self.font_size_heading_5
    }
    fn font_weight_regular(&self) -> f32 {
        self.font_weight_regular
    }
    fn font_weight_medium(&self) -> f32 {
        self.font_weight_medium
    }
    fn font_weight_semibold(&self) -> f32 {
        self.font_weight_semibold
    }
    fn font_weight_bold(&self) -> f32 {
        self.font_weight_bold
    }
    fn line_height(&self) -> f32 {
        self.line_height
    }
}

impl ISpacingTokens for DesignTokens {
    fn padding_xss(&self) -> f32 {
        self.padding_xss
    }
    fn padding_xs(&self) -> f32 {
        self.padding_xs
    }
    fn padding_sm(&self) -> f32 {
        self.padding_sm
    }
    fn padding(&self) -> f32 {
        self.padding
    }
    fn padding_md(&self) -> f32 {
        self.padding_md
    }
    fn padding_lg(&self) -> f32 {
        self.padding_lg
    }
    fn padding_xl(&self) -> f32 {
        self.padding_xl
    }
    fn border_radius(&self) -> f32 {
        self.border_radius
    }
    fn border_radius_sm(&self) -> f32 {
        self.border_radius_sm
    }
    fn border_radius_lg(&self) -> f32 {
        self.border_radius_lg
    }
    fn border_radius_xl(&self) -> f32 {
        self.border_radius_xl
    }
    fn border_radius_round(&self) -> f32 {
        self.border_radius_round
    }
    fn control_height_sm(&self) -> f32 {
        self.control_height_sm
    }
    fn control_height(&self) -> f32 {
        self.control_height
    }
    fn control_height_lg(&self) -> f32 {
        self.control_height_lg
    }
    fn motion_duration_fast(&self) -> f32 {
        self.motion_duration_fast
    }
    fn motion_duration_mid(&self) -> f32 {
        self.motion_duration_mid
    }
    fn motion_duration_slow(&self) -> f32 {
        self.motion_duration_slow
    }
    fn motion_easing_default(&self) -> &str {
        self.motion_easing_default
    }
    fn motion_easing_in(&self) -> &str {
        self.motion_easing_in
    }
    fn motion_easing_out(&self) -> &str {
        self.motion_easing_out
    }
    fn motion_easing_in_out(&self) -> &str {
        self.motion_easing_in_out
    }
    fn screen_xs(&self) -> f32 {
        self.screen_xs
    }
    fn screen_sm(&self) -> f32 {
        self.screen_sm
    }
    fn screen_md(&self) -> f32 {
        self.screen_md
    }
    fn screen_lg(&self) -> f32 {
        self.screen_lg
    }
    fn screen_xl(&self) -> f32 {
        self.screen_xl
    }
    fn screen_xxl(&self) -> f32 {
        self.screen_xxl
    }
}

impl IBoxShadowTokens for DesignTokens {
    fn box_shadow(&self) -> ShadowToken {
        self.box_shadow
    }
    fn box_shadow_secondary(&self) -> ShadowToken {
        self.box_shadow_secondary
    }
}

impl ThemeTokens for DesignTokens {
    fn is_dark(&self) -> bool {
        self.is_dark
    }
}

impl TokenProvider for DesignTokens {}
