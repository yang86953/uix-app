//! NavItem 与兼容 Navigation 构建器的 UIX 静态视觉契约。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavLayoutVisual {
    pub(super) default_width: f32,
    pub(super) default_item_height: f32,
    pub(super) default_shell_height: f32,
    pub(super) compact_min_width: f32,
    pub(super) paint_vertical_expand: f32,
    pub(super) paint_height_expand: f32,
    pub(super) indicator_width: f32,
    pub(super) indicator_radius: f32,
    pub(super) icon_start: f32,
    pub(super) icon_slot_width: f32,
    pub(super) icon_advance: f32,
    pub(super) active_label_start: f32,
    pub(super) label_start: f32,
    pub(super) toggle_height: f32,
    pub(super) title_height: f32,
    pub(super) version_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavTypographyVisual {
    label: NavFontRole,
    pub(super) icon: f32,
    pub(super) compact_icon: f32,
    pub(super) compact_fallback: f32,
    pub(super) title: f32,
    pub(super) version: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavChromeVisual {
    pub(super) focus_width: f32,
    radius: NavRadiusRole,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavDefaultsVisual {
    pub(super) show_version: bool,
    pub(super) show_title: bool,
    pub(super) compact_items: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct NavPaletteVisual {
    primary: ColorValue,
    primary_background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    fill_tertiary: ColorValue,
    container_background: ColorValue,
    elevated_background: ColorValue,
    border_secondary: ColorValue,
    text_quaternary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavFontRole {
    Base,
}

impl NavFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Base => tokens.font_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum NavRadiusRole {
    Small,
}

impl NavRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct NavVisual {
    pub(super) layout: NavLayoutVisual,
    pub(super) typography: NavTypographyVisual,
    pub(super) chrome: NavChromeVisual,
    pub(super) defaults: NavDefaultsVisual,
    palette: NavPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/nav/nav.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedNavVisual {
    pub(super) primary: Color,
    pub(super) primary_background: Color,
    pub(super) text: Color,
    pub(super) text_secondary: Color,
    pub(super) fill_tertiary: Color,
    pub(super) container_background: Color,
    pub(super) elevated_background: Color,
    pub(super) border_secondary: Color,
    pub(super) text_quaternary: Color,
    pub(super) label_font_size: f32,
    pub(super) radius: f32,
}

impl NavVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedNavVisual {
        ResolvedNavVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_background: self.palette.primary_background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            container_background: self.palette.container_background.resolve(tokens),
            elevated_background: self.palette.elevated_background.resolve(tokens),
            border_secondary: self.palette.border_secondary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            label_font_size: self.typography.label.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(super) const fn nav_font_base() -> NavFontRole {
    NavFontRole::Base
}
pub(super) const fn nav_radius_small() -> NavRadiusRole {
    NavRadiusRole::Small
}
pub(super) const fn nav_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn nav_primary_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(super) const fn nav_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn nav_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn nav_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(super) const fn nav_container_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(super) const fn nav_elevated_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(super) const fn nav_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
pub(super) const fn nav_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
