//! uix-ui theme 模块集成测试。

use uix_ui::api::traits::{IBoxShadowTokens, TokenProvider};
use uix_ui::api::{DesignTokens, ShadowToken};
use uix_graphics::color::Color;

// ════════════════════════════════════════════════════════════════════════════
// design_tokens 测试
// ════════════════════════════════════════════════════════════════════════════

#[test]
fn antd_light_is_not_dark() {
    let tk = DesignTokens::antd_light();
    assert!(!tk.is_dark());
    assert_eq!(tk.is_dark, false);
}

#[test]
fn antd_dark_is_dark() {
    let tk = DesignTokens::antd_dark();
    assert!(tk.is_dark());
    assert_eq!(tk.is_dark, true);
}

#[test]
fn antd_light_primary_color() {
    let tk = DesignTokens::antd_light();
    assert_eq!(tk.color_primary, Color::from_rgba(22, 119, 255, 255));
}

#[test]
fn antd_light_bg_colors() {
    let tk = DesignTokens::antd_light();
    assert!(tk.color_bg_container.luminance() > 240);
    assert!(tk.color_bg_layout.luminance() > 230);
    assert_eq!(tk.color_bg_elevated, Color::from_rgb(255, 255, 255));
}

#[test]
fn antd_dark_bg_colors() {
    let tk = DesignTokens::antd_dark();
    assert!(tk.color_bg_container.luminance() < 40);
    assert!(tk.color_bg_layout.luminance() < 30);
    assert!(tk.color_bg_elevated.luminance() < 60);
}

#[test]
fn light_text_colors() {
    let tk = DesignTokens::antd_light();
    assert_eq!(tk.color_text, Color::from_rgba(0, 0, 0, 224));
    assert_eq!(tk.color_white, Color::from_rgba(255, 255, 255, 255));
    assert_eq!(tk.color_black, Color::from_rgba(0, 0, 0, 255));
}

#[test]
fn dark_text_colors() {
    let tk = DesignTokens::antd_dark();
    assert_eq!(tk.color_text, Color::from_rgba(255, 255, 255, 224));
}

#[test]
fn semantic_colors_light() {
    let tk = DesignTokens::antd_light();
    assert_eq!(tk.color_success, Color::from_rgba(82, 196, 26, 255));
    assert_eq!(tk.color_warning, Color::from_rgba(250, 173, 20, 255));
    assert_eq!(tk.color_error, Color::from_rgba(255, 77, 79, 255));
    assert_eq!(tk.color_info, Color::from_rgba(22, 119, 255, 255));
}

#[test]
fn semantic_colors_dark() {
    let tk = DesignTokens::antd_dark();
    assert_eq!(tk.color_success, Color::from_rgb(73, 185, 22));
    assert_eq!(tk.color_warning, Color::from_rgb(250, 173, 20));
    assert_eq!(tk.color_error, Color::from_rgb(255, 77, 79));
    assert_eq!(tk.color_info, Color::from_rgb(22, 119, 255));
}

#[test]
fn typography_tokens_light() {
    let tk = DesignTokens::antd_light();
    assert_eq!(tk.font_family, "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial");
    assert_eq!(tk.font_size, 14.0);
    assert_eq!(tk.font_size_heading_1, 38.0);
    assert_eq!(tk.font_weight_regular, 400.0);
    assert_eq!(tk.font_weight_bold, 700.0);
}

#[test]
fn spacing_tokens_light() {
    let tk = DesignTokens::antd_light();
    assert_eq!(tk.padding, 16.0);
    assert_eq!(tk.padding_xs, 8.0);
    assert_eq!(tk.padding_lg, 24.0);
    assert_eq!(tk.border_radius, 6.0);
    assert_eq!(tk.control_height, 32.0);
}

#[test]
fn box_shadow_tokens_light() {
    let tk = DesignTokens::antd_light();
    let shadow = tk.box_shadow();
    assert!(shadow.layer_1.2 > 0.0);
    assert!(shadow.layer_2.2 > 0.0);
    assert!(shadow.layer_3.2 > 0.0);
}

#[test]
fn shadow_token_none() {
    let s = ShadowToken::none();
    assert_eq!(s.layer_1.2, 0.0);
    assert_eq!(s.layer_2.2, 0.0);
    assert_eq!(s.layer_3.2, 0.0);
}

#[test]
fn design_tokens_clone() {
    let tk1 = DesignTokens::antd_light();
    let tk2 = tk1.clone();
    assert_eq!(tk1.color_primary, tk2.color_primary);
    assert_eq!(tk1.is_dark, tk2.is_dark);
}
