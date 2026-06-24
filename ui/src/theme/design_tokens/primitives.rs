//! ThemePrimitives — 从几组核心颜色推导整套 DesignTokens。
//!
//! 设计目标：改 `primitives` 中的几种颜色即可切换整套视觉风格。
//!
//! 推导公式（RGB 线性插值）：
//! - primary_hover  = primary → white (15%)
//! - primary_active = primary → black (15%)
//! - primary_bg     = primary → white (85%)
//! - primary_border = primary → white (55%)
//! - success/warning/error/info bg/border 同上
//! - text_secondary  = text   × alpha 0.65
//! - text_tertiary   = text   × alpha 0.45
//! - text_quaternary = text   × alpha 0.25
//! - fill            = text   × alpha 0.12
//! - fill_secondary  = text   × alpha 0.06
//! - fill_tertiary   = text   × alpha 0.04
//! - fill_quaternary = text   × alpha 0.02
//! - border_secondary = border → white (50%)
//! - bg_container    = bg → white (差异 2%)
//! - bg_elevated     = bg → white (10-100%)
//! - bg_layout       = bg → black (差异 3%)
//! - bg_overlay      = bg → white (15-100%)
//! - link 系列      = color_primary 系列一致

use uix_graphics::Color;

use super::DesignTokens;

/// 主题核心基色——修改这些即可切换整套配色方案。
#[derive(Debug, Clone)]
pub struct ThemePrimitives {
    // ── 品牌色 ──
    pub primary: Color,

    // ── 语义色 ──
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub info: Color,

    // ── 中性色（随 dark/light 模式切换）──
    pub bg: Color,
    pub text: Color,
    pub border: Color,
}

impl ThemePrimitives {
    /// Ant Design 5 亮色主题基色。
    pub fn antd_light() -> Self {
        Self {
            primary: Color::from_rgb(22, 119, 255),
            success: Color::from_rgb(82, 196, 26),
            warning: Color::from_rgb(250, 173, 20),
            error: Color::from_rgb(255, 77, 79),
            info: Color::from_rgb(22, 119, 255),
            bg: Color::from_rgb(245, 245, 247),
            text: Color::from_rgb(0, 0, 0),
            border: Color::from_rgb(228, 228, 231),
        }
    }

    /// Ant Design 5 暗色主题基色。
    pub fn antd_dark() -> Self {
        Self {
            primary: Color::from_rgb(22, 119, 255),
            success: Color::from_rgb(73, 185, 22),
            warning: Color::from_rgb(250, 173, 20),
            error: Color::from_rgb(255, 77, 79),
            info: Color::from_rgb(22, 119, 255),
            bg: Color::from_rgb(21, 21, 21),
            text: Color::from_rgb(255, 255, 255),
            border: Color::from_rgb(56, 56, 58),
        }
    }

    /// 从基色推导完整的 DesignTokens。
    pub fn into_design_tokens(self, is_dark: bool) -> DesignTokens {
        let p = &self;
        let w = Color::white();
        let b = Color::black();

        DesignTokens {
            // ── 品牌色 ──
            color_primary: p.primary,
            color_primary_hover: p.primary.lighten(0.15),
            color_primary_active: p.primary.darken(0.15),
            color_primary_bg: p.primary.mix(&w, 0.85),
            color_primary_border: p.primary.mix(&w, 0.55),

            // ── 中性背景 ──
            color_bg_container: if is_dark { p.bg.lighten(0.04) } else { p.bg.mix(&w, 0.55) },
            color_bg_elevated: if is_dark { p.bg.lighten(0.12) } else { w },
            color_bg_raised: if is_dark { p.bg.lighten(0.08) } else { p.bg.mix(&w, 0.35) },
            color_bg_overlay: if is_dark { p.bg.lighten(0.18) } else { w },
            color_bg_layout: if is_dark { p.bg.darken(0.03) } else { p.bg.mix(&w, 0.2) },
            color_bg_spotlight: b,
            color_bg_mask: if is_dark {
                Color::from_rgba(0, 0, 0, 166)
            } else {
                Color::from_rgba(0, 0, 0, 115)
            },

            // ── 边框 ──
            color_border: p.border,
            color_border_secondary: p.border.mix(&w, 0.5),

            // ── 填充（用于 hover/pressed/disabled 背景色，以 text 为基乘以小 alpha）──
            color_fill: p.text.with_alpha(31),
            color_fill_secondary: p.text.with_alpha(15),
            color_fill_tertiary: p.text.with_alpha(10),
            color_fill_quaternary: p.text.with_alpha(5),

            // ── 文字 ──
            color_text: p.text.with_alpha(224),
            color_text_secondary: p.text.with_alpha(153),
            color_text_tertiary: p.text.with_alpha(102),
            color_text_quaternary: p.text.with_alpha(51),
            color_white: w,
            color_black: b,

            // ── 阴影（以 text 为基）──
            color_shadow: p.text.with_alpha(20),
            color_shadow_secondary: p.text.with_alpha(10),

            // ── 语义色 ──
            color_success: p.success,
            color_success_bg: p.success.mix(&w, 0.85),
            color_success_border: p.success.mix(&w, 0.55),
            color_warning: p.warning,
            color_warning_bg: p.warning.mix(&w, 0.80),
            color_warning_border: p.warning.mix(&w, 0.50),
            color_error: p.error,
            color_error_bg: p.error.mix(&w, 0.82),
            color_error_border: p.error.mix(&w, 0.50),
            color_info: p.primary,
            color_info_bg: p.primary.mix(&w, 0.85),
            color_info_border: p.primary.mix(&w, 0.55),

            // ── 链接（与 primary 一致）──
            color_link: p.primary,
            color_link_hover: p.primary.lighten(0.15),
            color_link_active: p.primary.darken(0.15),

            // ── 以下为非颜色字段，沿用例值 ──
            ..design_tokens_defaults(is_dark)
        }
    }
}

/// 非颜色字段的默认值。
fn design_tokens_defaults(is_dark: bool) -> DesignTokens {
    DesignTokens {
        font_family: "-apple-system, BlinkMacSystemFont, Segoe UI, Roboto, Helvetica Neue, Arial",
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
        box_shadow: crate::theme::color_tokens::ShadowToken {
            layer_1: (0.0, 2.0, 8.0, Color::from_rgba(0, 0, 0, 24)),
            layer_2: (0.0, 4.0, 16.0, Color::from_rgba(0, 0, 0, 20)),
            layer_3: (0.0, 8.0, 32.0, Color::from_rgba(0, 0, 0, 16)),
        },
        box_shadow_secondary: crate::theme::color_tokens::ShadowToken {
            layer_1: (0.0, 1.0, 4.0, Color::from_rgba(0, 0, 0, 16)),
            layer_2: (0.0, 2.0, 8.0, Color::from_rgba(0, 0, 0, 12)),
            layer_3: (0.0, 4.0, 16.0, Color::from_rgba(0, 0, 0, 8)),
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
        is_dark,
        // 颜色字段用 dummy 值，实际通过 into_design_tokens 覆盖
        color_primary: Color::black(),
        color_primary_hover: Color::black(),
        color_primary_active: Color::black(),
        color_primary_bg: Color::black(),
        color_primary_border: Color::black(),
        color_bg_container: Color::black(),
        color_bg_elevated: Color::black(),
        color_bg_raised: Color::black(),
        color_bg_overlay: Color::black(),
        color_bg_layout: Color::black(),
        color_bg_spotlight: Color::black(),
        color_bg_mask: Color::black(),
        color_border: Color::black(),
        color_border_secondary: Color::black(),
        color_fill: Color::black(),
        color_fill_secondary: Color::black(),
        color_fill_tertiary: Color::black(),
        color_fill_quaternary: Color::black(),
        color_text: Color::black(),
        color_text_secondary: Color::black(),
        color_text_tertiary: Color::black(),
        color_text_quaternary: Color::black(),
        color_white: Color::black(),
        color_black: Color::black(),
        color_shadow: Color::black(),
        color_shadow_secondary: Color::black(),
        color_success: Color::black(),
        color_success_bg: Color::black(),
        color_success_border: Color::black(),
        color_warning: Color::black(),
        color_warning_bg: Color::black(),
        color_warning_border: Color::black(),
        color_error: Color::black(),
        color_error_bg: Color::black(),
        color_error_border: Color::black(),
        color_info: Color::black(),
        color_info_bg: Color::black(),
        color_info_border: Color::black(),
        color_link: Color::black(),
        color_link_hover: Color::black(),
        color_link_active: Color::black(),
    }
}
