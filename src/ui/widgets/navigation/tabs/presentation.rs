//! Tabs 的 UIX 静态视觉契约与主题解析。

use super::TabPosition;
use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TabsLayoutVisual {
    pub(super) default_width: f32,
    pub(super) default_height: f32,
    pub(super) tab_height: f32,
    pub(super) gap: f32,
    pub(super) horizontal_padding: f32,
    pub(super) side_bar_width: f32,
    pub(super) wheel_step: f32,
    pub(super) icon_reserve: f32,
    pub(super) close_reserve: f32,
    pub(super) icon_slot_width: f32,
    pub(super) icon_advance: f32,
    pub(super) add_size: f32,
    pub(super) close_size: f32,
    pub(super) divider_thickness: f32,
    pub(super) indicator_extent_factor: f32,
    pub(super) indicator_thickness: f32,
    pub(super) content_horizontal_inset: f32,
    pub(super) content_vertical_inset: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TabsTypographyVisual {
    label: TabsFontRole,
    pub(super) tab_icon: f32,
    pub(super) close_icon: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TabsChromeVisual {
    pub(super) indicator_radius: f32,
    pub(super) focus_width: f32,
    radius: TabsRadiusRole,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TabsDefaultsVisual {
    pub(super) position: TabPosition,
    pub(super) editable: bool,
    pub(super) scrollable: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TabsIconsVisual {
    pub(super) close: &'static str,
    pub(super) add: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TabsPaletteVisual {
    container_background: ColorValue,
    border_secondary: ColorValue,
    primary: ColorValue,
    text_secondary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabsFontRole {
    Base,
}

impl TabsFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Base => tokens.font_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TabsRadiusRole {
    Small,
}

impl TabsRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct TabsVisual {
    pub(super) layout: TabsLayoutVisual,
    pub(super) typography: TabsTypographyVisual,
    pub(super) chrome: TabsChromeVisual,
    pub(super) defaults: TabsDefaultsVisual,
    pub(super) icons: TabsIconsVisual,
    palette: TabsPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/tabs/tabs.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedTabsVisual {
    pub(super) container_background: Color,
    pub(super) border_secondary: Color,
    pub(super) primary: Color,
    pub(super) text_secondary: Color,
    pub(super) label_font_size: f32,
    pub(super) radius: f32,
}

impl TabsVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedTabsVisual {
        ResolvedTabsVisual {
            container_background: self.palette.container_background.resolve(tokens),
            border_secondary: self.palette.border_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            label_font_size: self.typography.label.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(super) const fn tabs_position_top() -> TabPosition {
    TabPosition::Top
}
pub(super) const fn tabs_font_base() -> TabsFontRole {
    TabsFontRole::Base
}
pub(super) const fn tabs_radius_small() -> TabsRadiusRole {
    TabsRadiusRole::Small
}
pub(super) const fn tabs_container_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(super) const fn tabs_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
pub(super) const fn tabs_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn tabs_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
