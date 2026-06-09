//! DesignTokens — concrete Ant Design 5 token preset with light & dark modes.
//!
//! Implements all token sub-traits (`IColorTokens`, `ITypographyTokens`,
//! `ISpacingTokens`, `IBoxShadowTokens`, `TokenProvider`) by delegating to
//! public fields. Factory functions `antd_light()` and `antd_dark()` provide
//! complete Ant Design 5 presets.

use super::color_tokens::IColorTokens;
use super::color_tokens::ShadowToken;
use super::spacing_tokens::{IBoxShadowTokens, ISpacingTokens};
use super::wrapper::TokenProvider;
use super::typography_tokens::ITypographyTokens;
use crate::graphics::Color;

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

impl DesignTokens {
    pub fn antd_light() -> Self {
        Self {
            color_primary: Color::from_rgb(22, 119, 255),
            color_primary_hover: Color::from_rgb(64, 150, 255),
            color_primary_active: Color::from_rgb(9, 88, 217),
            color_primary_bg: Color::from_rgb(230, 244, 255),
            color_primary_border: Color::from_rgb(186, 224, 255),
            color_bg_container: Color::from_rgb(250, 250, 252),
            color_bg_elevated: Color::from_rgb(255, 255, 255),
            color_bg_raised: Color::from_rgb(245, 245, 247),
            color_bg_overlay: Color::from_rgb(255, 255, 255),
            color_bg_layout: Color::from_rgb(242, 242, 245),
            color_bg_spotlight: Color::from_rgb(0, 0, 0),
            color_bg_mask: Color::from_rgba(0, 0, 0, 115),
            color_border: Color::from_rgb(228, 228, 231),
            color_border_secondary: Color::from_rgb(240, 240, 242),
            color_fill: Color::from_rgba(0, 0, 0, 31),
            color_fill_secondary: Color::from_rgba(0, 0, 0, 15),
            color_fill_tertiary: Color::from_rgba(0, 0, 0, 10),
            color_fill_quaternary: Color::from_rgba(0, 0, 0, 5),
            color_text: Color::from_rgba(0, 0, 0, 224),
            color_text_secondary: Color::from_rgba(0, 0, 0, 153),
            color_text_tertiary: Color::from_rgba(0, 0, 0, 102),
            color_text_quaternary: Color::from_rgba(0, 0, 0, 51),
            color_white: Color::white(),
            color_black: Color::black(),
            color_shadow: Color::from_rgba(0, 0, 0, 20),
            color_shadow_secondary: Color::from_rgba(0, 0, 0, 10),
            color_success: Color::from_rgb(82, 196, 26),
            color_success_bg: Color::from_rgb(246, 255, 237),
            color_success_border: Color::from_rgb(183, 235, 143),
            color_warning: Color::from_rgb(250, 173, 20),
            color_warning_bg: Color::from_rgb(255, 251, 230),
            color_warning_border: Color::from_rgb(255, 229, 143),
            color_error: Color::from_rgb(255, 77, 79),
            color_error_bg: Color::from_rgb(255, 242, 240),
            color_error_border: Color::from_rgb(255, 188, 185),
            color_info: Color::from_rgb(22, 119, 255),
            color_info_bg: Color::from_rgb(230, 244, 255),
            color_info_border: Color::from_rgb(186, 224, 255),
            color_link: Color::from_rgb(22, 119, 255),
            color_link_hover: Color::from_rgb(64, 150, 255),
            color_link_active: Color::from_rgb(9, 88, 217),
            font_family:
                "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial",
            font_size_sm: 12.0,
            font_size: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_heading_1: 38.0,
            font_size_heading_2: 30.0,
            font_size_heading_3: 24.0,
            font_size_heading_4: 20.0,
            font_size_heading_5: 16.0,
            font_weight_regular: 400.0,
            font_weight_medium: 500.0,
            font_weight_semibold: 600.0,
            font_weight_bold: 700.0,
            line_height: 1.5715,
            padding_xss: 4.0,
            padding_xs: 8.0,
            padding_sm: 12.0,
            padding: 16.0,
            padding_md: 20.0,
            padding_lg: 24.0,
            padding_xl: 32.0,
            border_radius: 6.0,
            border_radius_sm: 4.0,
            border_radius_lg: 8.0,
            border_radius_xl: 12.0,
            border_radius_round: 999.0,
            control_height_sm: 24.0,
            control_height: 32.0,
            control_height_lg: 40.0,
            box_shadow: ShadowToken {
                layer_1: (0.0, 1.0, 2.0, Color::from_rgba(0, 0, 0, 15)),
                layer_2: (0.0, 1.0, 6.0, Color::from_rgba(0, 0, 0, 10)),
                layer_3: (0.0, 2.0, 16.0, Color::from_rgba(0, 0, 0, 8)),
            },
            box_shadow_secondary: ShadowToken {
                layer_1: (0.0, 6.0, 16.0, Color::from_rgba(0, 0, 0, 20)),
                layer_2: (0.0, 3.0, 6.0, Color::from_rgba(0, 0, 0, 10)),
                layer_3: (0.0, 9.0, 28.0, Color::from_rgba(0, 0, 0, 15)),
            },
            motion_duration_fast: 0.1,
            motion_duration_mid: 0.2,
            motion_duration_slow: 0.3,
            motion_easing_default: "cubic-bezier(0.25, 0.1, 0.25, 1)",
            motion_easing_in: "cubic-bezier(0.42, 0, 1, 1)",
            motion_easing_out: "cubic-bezier(0, 0, 0.58, 1)",
            motion_easing_in_out: "cubic-bezier(0.42, 0, 0.58, 1)",
            screen_xs: 480.0,
            screen_sm: 576.0,
            screen_md: 768.0,
            screen_lg: 992.0,
            screen_xl: 1200.0,
            screen_xxl: 1600.0,
            is_dark: false,
        }
    }

    pub fn antd_dark() -> Self {
        Self {
            color_primary: Color::from_rgb(22, 119, 255),
            color_primary_hover: Color::from_rgb(64, 150, 255),
            color_primary_active: Color::from_rgb(9, 88, 217),
            color_primary_bg: Color::from_rgb(17, 33, 58),
            color_primary_border: Color::from_rgb(40, 61, 92),
            color_bg_container: Color::from_rgb(30, 30, 30),
            color_bg_elevated: Color::from_rgb(42, 42, 45),
            color_bg_raised: Color::from_rgb(36, 36, 38),
            color_bg_overlay: Color::from_rgb(48, 48, 50),
            color_bg_layout: Color::from_rgb(21, 21, 21),
            color_bg_spotlight: Color::from_rgb(0, 0, 0),
            color_bg_mask: Color::from_rgba(0, 0, 0, 166),
            color_border: Color::from_rgb(56, 56, 58),
            color_border_secondary: Color::from_rgb(48, 48, 48),
            color_fill: Color::from_rgba(255, 255, 255, 31),
            color_fill_secondary: Color::from_rgba(255, 255, 255, 20),
            color_fill_tertiary: Color::from_rgba(255, 255, 255, 10),
            color_fill_quaternary: Color::from_rgba(255, 255, 255, 5),
            color_text: Color::from_rgba(255, 255, 255, 224),
            color_text_secondary: Color::from_rgba(255, 255, 255, 166),
            color_text_tertiary: Color::from_rgba(255, 255, 255, 115),
            color_text_quaternary: Color::from_rgba(255, 255, 255, 64),
            color_white: Color::white(),
            color_black: Color::black(),
            color_shadow: Color::from_rgba(0, 0, 0, 140),
            color_shadow_secondary: Color::from_rgba(0, 0, 0, 89),
            color_success: Color::from_rgb(73, 185, 22),
            color_success_bg: Color::from_rgb(24, 42, 23),
            color_success_border: Color::from_rgb(50, 82, 40),
            color_warning: Color::from_rgb(250, 173, 20),
            color_warning_bg: Color::from_rgb(49, 39, 18),
            color_warning_border: Color::from_rgb(87, 67, 30),
            color_error: Color::from_rgb(255, 77, 79),
            color_error_bg: Color::from_rgb(46, 27, 29),
            color_error_border: Color::from_rgb(82, 45, 47),
            color_info: Color::from_rgb(22, 119, 255),
            color_info_bg: Color::from_rgb(17, 33, 58),
            color_info_border: Color::from_rgb(40, 61, 92),
            color_link: Color::from_rgb(22, 119, 255),
            color_link_hover: Color::from_rgb(64, 150, 255),
            color_link_active: Color::from_rgb(9, 88, 217),
            font_family:
                "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial",
            font_size_sm: 12.0,
            font_size: 14.0,
            font_size_lg: 16.0,
            font_size_xl: 20.0,
            font_size_heading_1: 38.0,
            font_size_heading_2: 30.0,
            font_size_heading_3: 24.0,
            font_size_heading_4: 20.0,
            font_size_heading_5: 16.0,
            font_weight_regular: 400.0,
            font_weight_medium: 500.0,
            font_weight_semibold: 600.0,
            font_weight_bold: 700.0,
            line_height: 1.5715,
            padding_xss: 4.0,
            padding_xs: 8.0,
            padding_sm: 12.0,
            padding: 16.0,
            padding_md: 20.0,
            padding_lg: 24.0,
            padding_xl: 32.0,
            border_radius: 6.0,
            border_radius_sm: 4.0,
            border_radius_lg: 8.0,
            border_radius_xl: 12.0,
            border_radius_round: 999.0,
            control_height_sm: 24.0,
            control_height: 32.0,
            control_height_lg: 40.0,
            box_shadow: ShadowToken {
                layer_1: (0.0, 1.0, 2.0, Color::from_rgba(0, 0, 0, 45)),
                layer_2: (0.0, 1.0, 6.0, Color::from_rgba(0, 0, 0, 35)),
                layer_3: (0.0, 2.0, 16.0, Color::from_rgba(0, 0, 0, 25)),
            },
            box_shadow_secondary: ShadowToken {
                layer_1: (0.0, 6.0, 16.0, Color::from_rgba(0, 0, 0, 55)),
                layer_2: (0.0, 3.0, 6.0, Color::from_rgba(0, 0, 0, 35)),
                layer_3: (0.0, 9.0, 28.0, Color::from_rgba(0, 0, 0, 40)),
            },
            motion_duration_fast: 0.1,
            motion_duration_mid: 0.2,
            motion_duration_slow: 0.3,
            motion_easing_default: "cubic-bezier(0.25, 0.1, 0.25, 1)",
            motion_easing_in: "cubic-bezier(0.42, 0, 1, 1)",
            motion_easing_out: "cubic-bezier(0, 0, 0.58, 1)",
            motion_easing_in_out: "cubic-bezier(0.42, 0, 0.58, 1)",
            screen_xs: 480.0,
            screen_sm: 576.0,
            screen_md: 768.0,
            screen_lg: 992.0,
            screen_xl: 1200.0,
            screen_xxl: 1600.0,
            is_dark: true,
        }
    }
}

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

impl TokenProvider for DesignTokens {
    fn is_dark(&self) -> bool {
        self.is_dark
    }
}
