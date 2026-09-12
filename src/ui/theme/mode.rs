//! Runtime theme switching independent of a concrete palette.
use std::sync::atomic::{AtomicBool, Ordering};
use super::Theme;
use crate::draw::Color;
use crate::ui::{IColorTokens, ShadowToken, IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider};
#[derive(Debug)]
pub struct ModeTokens { light: Theme, dark_theme: Theme, dark: AtomicBool }
impl ModeTokens {
    pub fn new(light: Theme, dark: Theme, is_dark: bool) -> Self {
        Self { light, dark_theme: dark, dark: AtomicBool::new(is_dark) }
    }
    pub fn set_mode(&self, dark: bool) { self.dark.store(dark, Ordering::Release); }
    /// Returns the currently selected immutable theme.
    pub fn snapshot(&self) -> Theme {
        if self.dark.load(Ordering::Acquire) { self.dark_theme.clone() } else { self.light.clone() }
    }
    fn active(&self) -> &dyn TokenProvider {
        if self.dark.load(Ordering::Acquire) { self.dark_theme.tokens() } else { self.light.tokens() }
    }
}
impl IColorTokens for ModeTokens {
    fn color_primary(&self) -> Color {
        self.active().color_primary()
    }
    fn color_primary_hover(&self) -> Color {
        self.active().color_primary_hover()
    }
    fn color_primary_active(&self) -> Color {
        self.active().color_primary_active()
    }
    fn color_primary_bg(&self) -> Color {
        self.active().color_primary_bg()
    }
    fn color_primary_border(&self) -> Color {
        self.active().color_primary_border()
    }
    fn color_bg_container(&self) -> Color {
        self.active().color_bg_container()
    }
    fn color_bg_elevated(&self) -> Color {
        self.active().color_bg_elevated()
    }
    fn color_bg_raised(&self) -> Color {
        self.active().color_bg_raised()
    }
    fn color_bg_overlay(&self) -> Color {
        self.active().color_bg_overlay()
    }
    fn color_bg_layout(&self) -> Color {
        self.active().color_bg_layout()
    }
    fn color_bg_spotlight(&self) -> Color {
        self.active().color_bg_spotlight()
    }
    fn color_bg_mask(&self) -> Color {
        self.active().color_bg_mask()
    }
    fn color_border(&self) -> Color {
        self.active().color_border()
    }
    fn color_border_secondary(&self) -> Color {
        self.active().color_border_secondary()
    }
    fn color_fill(&self) -> Color {
        self.active().color_fill()
    }
    fn color_fill_secondary(&self) -> Color {
        self.active().color_fill_secondary()
    }
    fn color_fill_tertiary(&self) -> Color {
        self.active().color_fill_tertiary()
    }
    fn color_fill_quaternary(&self) -> Color {
        self.active().color_fill_quaternary()
    }
    fn color_text(&self) -> Color {
        self.active().color_text()
    }
    fn color_text_secondary(&self) -> Color {
        self.active().color_text_secondary()
    }
    fn color_text_tertiary(&self) -> Color {
        self.active().color_text_tertiary()
    }
    fn color_text_quaternary(&self) -> Color {
        self.active().color_text_quaternary()
    }
    fn color_white(&self) -> Color {
        self.active().color_white()
    }
    fn color_black(&self) -> Color {
        self.active().color_black()
    }
    fn color_shadow(&self) -> Color {
        self.active().color_shadow()
    }
    fn color_shadow_secondary(&self) -> Color {
        self.active().color_shadow_secondary()
    }
    fn color_success(&self) -> Color {
        self.active().color_success()
    }
    fn color_success_bg(&self) -> Color {
        self.active().color_success_bg()
    }
    fn color_success_border(&self) -> Color {
        self.active().color_success_border()
    }
    fn color_warning(&self) -> Color {
        self.active().color_warning()
    }
    fn color_warning_bg(&self) -> Color {
        self.active().color_warning_bg()
    }
    fn color_warning_border(&self) -> Color {
        self.active().color_warning_border()
    }
    fn color_error(&self) -> Color {
        self.active().color_error()
    }
    fn color_error_bg(&self) -> Color {
        self.active().color_error_bg()
    }
    fn color_error_border(&self) -> Color {
        self.active().color_error_border()
    }
    fn color_info(&self) -> Color {
        self.active().color_info()
    }
    fn color_info_bg(&self) -> Color {
        self.active().color_info_bg()
    }
    fn color_info_border(&self) -> Color {
        self.active().color_info_border()
    }
    fn color_link(&self) -> Color {
        self.active().color_link()
    }
    fn color_link_hover(&self) -> Color {
        self.active().color_link_hover()
    }
    fn color_link_active(&self) -> Color {
        self.active().color_link_active()
    }
}

impl ITypographyTokens for ModeTokens {
    // &str 字段在 antd_light 和 antd_dark 中值相同，直接返回静态字符串
    fn font_family(&self) -> &str { self.active().font_family() }
    fn font_size_sm(&self) -> f32 {
        self.active().font_size_sm()
    }
    fn font_size(&self) -> f32 {
        self.active().font_size()
    }
    fn font_size_lg(&self) -> f32 {
        self.active().font_size_lg()
    }
    fn font_size_xl(&self) -> f32 {
        self.active().font_size_xl()
    }
    fn font_size_heading_1(&self) -> f32 {
        self.active().font_size_heading_1()
    }
    fn font_size_heading_2(&self) -> f32 {
        self.active().font_size_heading_2()
    }
    fn font_size_heading_3(&self) -> f32 {
        self.active().font_size_heading_3()
    }
    fn font_size_heading_4(&self) -> f32 {
        self.active().font_size_heading_4()
    }
    fn font_size_heading_5(&self) -> f32 {
        self.active().font_size_heading_5()
    }
    fn font_weight_regular(&self) -> f32 {
        self.active().font_weight_regular()
    }
    fn font_weight_medium(&self) -> f32 {
        self.active().font_weight_medium()
    }
    fn font_weight_semibold(&self) -> f32 {
        self.active().font_weight_semibold()
    }
    fn font_weight_bold(&self) -> f32 {
        self.active().font_weight_bold()
    }
    fn line_height(&self) -> f32 {
        self.active().line_height()
    }
}

impl ISpacingTokens for ModeTokens {
    fn padding_xss(&self) -> f32 {
        self.active().padding_xss()
    }
    fn padding_xs(&self) -> f32 {
        self.active().padding_xs()
    }
    fn padding_sm(&self) -> f32 {
        self.active().padding_sm()
    }
    fn padding(&self) -> f32 {
        self.active().padding()
    }
    fn padding_md(&self) -> f32 {
        self.active().padding_md()
    }
    fn padding_lg(&self) -> f32 {
        self.active().padding_lg()
    }
    fn padding_xl(&self) -> f32 {
        self.active().padding_xl()
    }
    fn border_radius(&self) -> f32 {
        self.active().border_radius()
    }
    fn border_radius_sm(&self) -> f32 {
        self.active().border_radius_sm()
    }
    fn border_radius_lg(&self) -> f32 {
        self.active().border_radius_lg()
    }
    fn border_radius_xl(&self) -> f32 {
        self.active().border_radius_xl()
    }
    fn border_radius_round(&self) -> f32 {
        self.active().border_radius_round()
    }
    fn control_height_sm(&self) -> f32 {
        self.active().control_height_sm()
    }
    fn control_height(&self) -> f32 {
        self.active().control_height()
    }
    fn control_height_lg(&self) -> f32 {
        self.active().control_height_lg()
    }
    // motion & screen 字段在 antd_light/dark 中值相同，也使用 RwLock
    fn motion_duration_fast(&self) -> f32 {
        self.active().motion_duration_fast()
    }
    fn motion_duration_mid(&self) -> f32 {
        self.active().motion_duration_mid()
    }
    fn motion_duration_slow(&self) -> f32 {
        self.active().motion_duration_slow()
    }
    fn motion_easing_default(&self) -> &str { self.active().motion_easing_default() }
    fn motion_easing_in(&self) -> &str { self.active().motion_easing_in() }
    fn motion_easing_out(&self) -> &str { self.active().motion_easing_out() }
    fn motion_easing_in_out(&self) -> &str { self.active().motion_easing_in_out() }
    fn screen_xs(&self) -> f32 {
        self.active().screen_xs()
    }
    fn screen_sm(&self) -> f32 {
        self.active().screen_sm()
    }
    fn screen_md(&self) -> f32 {
        self.active().screen_md()
    }
    fn screen_lg(&self) -> f32 {
        self.active().screen_lg()
    }
    fn screen_xl(&self) -> f32 {
        self.active().screen_xl()
    }
    fn screen_xxl(&self) -> f32 {
        self.active().screen_xxl()
    }
}

impl IBoxShadowTokens for ModeTokens {
    fn box_shadow(&self) -> ShadowToken {
        self.active().box_shadow()
    }
    fn box_shadow_secondary(&self) -> ShadowToken {
        self.active().box_shadow_secondary()
    }
}

impl ThemeTokens for ModeTokens {
    fn is_dark(&self) -> bool {
        self.dark.load(std::sync::atomic::Ordering::Acquire)
    }
}

impl TokenProvider for ModeTokens {}
