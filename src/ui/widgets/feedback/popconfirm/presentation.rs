//! Popconfirm 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};

use super::PopconfirmPlacement;

// 保存气泡、兼容触发器、默认行为和静态按钮文案。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopconfirmDefaultsVisual {
    pub(crate) popup_width: f32,
    pub(crate) popup_height: f32,
    pub(crate) trigger_width: f32,
    pub(crate) trigger_height: f32,
    pub(crate) placement: PopconfirmPlacement,
    pub(crate) arrow: bool,
    pub(crate) icon: bool,
    pub(crate) confirm_text: &'static str,
    pub(crate) cancel_text: &'static str,
}

// 保存定位、内容、按钮、箭头和脏区使用的全部静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopconfirmLayoutVisual {
    pub(crate) arrow_gap: f32,
    pub(crate) plain_gap: f32,
    pub(crate) fallback_offset_popups: f32,
    pub(crate) fallback_span_popups: f32,
    pub(crate) shadow_expand: f32,
    pub(crate) content_inset: f32,
    pub(crate) title_button_gap: f32,
    pub(crate) title_top_inset: f32,
    pub(crate) icon_min_popup_width: f32,
    pub(crate) icon_width: f32,
    pub(crate) icon_top_inset: f32,
    pub(crate) button_gap: f32,
    pub(crate) button_height: f32,
    pub(crate) button_bottom_inset: f32,
    pub(crate) arrow_size: f32,
}

// 保存兼容触发器、标题、按钮和图标使用的排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopconfirmTypographyVisual {
    trigger: PopconfirmFontRole,
    title: PopconfirmFontRole,
    confirm: PopconfirmFontRole,
    cancel: PopconfirmFontRole,
    pub(crate) warning_icon: f32,
}

// 保存默认进入与退出时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopconfirmMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) exit_duration: f64,
}

// 保存警告图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PopconfirmIconsVisual {
    pub(crate) warning: &'static str,
}

// 保存字体令牌或固定字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PopconfirmFontRole {
    Small,
    Fixed(f32),
}

impl PopconfirmFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.font_size_sm(),
            Self::Fixed(value) => value,
        }
    }
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopconfirmRadiusRole {
    Small,
}

impl PopconfirmRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存主题阴影角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopconfirmShadowRole {
    Secondary,
}

impl PopconfirmShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Secondary => tokens.box_shadow_secondary(),
        }
    }
}

// 保存描边、浮层层级、圆角和阴影角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopconfirmChromeVisual {
    pub(crate) panel_stroke: f32,
    pub(crate) focus_stroke: f32,
    pub(crate) overlay_z: i32,
    radius: PopconfirmRadiusRole,
    pub(crate) button_radius: f32,
    shadow: PopconfirmShadowRole,
}

// 保存 Popconfirm 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PopconfirmPaletteVisual {
    popup_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    primary: ColorValue,
    primary_hover: ColorValue,
    primary_active: ColorValue,
    error: ColorValue,
    warning: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    white: ColorValue,
}

// 全部 Popconfirm 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopconfirmVisual {
    pub(crate) defaults: PopconfirmDefaultsVisual,
    pub(crate) layout: PopconfirmLayoutVisual,
    pub(crate) typography: PopconfirmTypographyVisual,
    pub(crate) motion: PopconfirmMotionVisual,
    pub(crate) icons: PopconfirmIconsVisual,
    pub(crate) chrome: PopconfirmChromeVisual,
    palette: PopconfirmPaletteVisual,
}

crate::uix_items!("src/ui/widgets/feedback/popconfirm/popconfirm.uix");

// 保存每帧一次解析得到的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedPopconfirmVisual {
    pub(crate) popup_background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) primary_active: Color,
    pub(crate) error: Color,
    pub(crate) warning: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) white: Color,
    pub(crate) radius: f32,
    pub(crate) button_radius: f32,
    pub(crate) shadow: ShadowToken,
    pub(crate) trigger_font_size: f32,
    pub(crate) title_font_size: f32,
    pub(crate) confirm_font_size: f32,
    pub(crate) cancel_font_size: f32,
}

impl PopconfirmVisual {
    // 隐藏气泡不解析只供弹出内容使用的主题值。
    pub(crate) fn resolve(
        &self,
        tokens: &dyn ThemeTokens,
        popup_present: bool,
    ) -> ResolvedPopconfirmVisual {
        let (
            popup_background,
            border,
            text,
            primary_hover,
            primary_active,
            warning,
            white,
            button_radius,
            shadow,
            title_font_size,
            confirm_font_size,
            cancel_font_size,
        ) = if popup_present {
            (
                self.palette.popup_background.resolve(tokens),
                self.palette.border.resolve(tokens),
                self.palette.text.resolve(tokens),
                self.palette.primary_hover.resolve(tokens),
                self.palette.primary_active.resolve(tokens),
                self.palette.warning.resolve(tokens),
                self.palette.white.resolve(tokens),
                self.chrome.button_radius,
                self.chrome.shadow.resolve(tokens),
                self.typography.title.resolve(tokens),
                self.typography.confirm.resolve(tokens),
                self.typography.cancel.resolve(tokens),
            )
        } else {
            (
                Color::transparent(),
                Color::transparent(),
                Color::transparent(),
                Color::transparent(),
                Color::transparent(),
                Color::transparent(),
                Color::transparent(),
                0.0,
                ShadowToken::none(),
                0.0,
                0.0,
                0.0,
            )
        };
        ResolvedPopconfirmVisual {
            popup_background,
            border,
            text,
            primary: self.palette.primary.resolve(tokens),
            primary_hover,
            primary_active,
            error: self.palette.error.resolve(tokens),
            warning,
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            white,
            radius: self.chrome.radius.resolve(tokens),
            button_radius,
            shadow,
            trigger_font_size: self.typography.trigger.resolve(tokens),
            title_font_size,
            confirm_font_size,
            cancel_font_size,
        }
    }
}

pub(crate) const fn popconfirm_placement_top() -> PopconfirmPlacement {
    PopconfirmPlacement::Top
}
pub(crate) const fn popconfirm_font_small() -> PopconfirmFontRole {
    PopconfirmFontRole::Small
}
pub(crate) const fn popconfirm_font_fixed(value: f32) -> PopconfirmFontRole {
    PopconfirmFontRole::Fixed(value)
}
pub(crate) const fn popconfirm_radius_small() -> PopconfirmRadiusRole {
    PopconfirmRadiusRole::Small
}
pub(crate) const fn popconfirm_shadow_secondary() -> PopconfirmShadowRole {
    PopconfirmShadowRole::Secondary
}
pub(crate) const fn popconfirm_bg_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn popconfirm_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn popconfirm_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn popconfirm_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn popconfirm_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
pub(crate) const fn popconfirm_primary_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryActive)
}
pub(crate) const fn popconfirm_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(crate) const fn popconfirm_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn popconfirm_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn popconfirm_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn popconfirm_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
