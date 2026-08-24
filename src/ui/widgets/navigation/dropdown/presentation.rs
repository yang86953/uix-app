//! Dropdown 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DropdownLayoutVisual {
    pub(super) width: f32,
    pub(super) trigger_height: f32,
    pub(super) row_height: f32,
    pub(super) divider_row_height: f32,
    pub(super) content_start: f32,
    pub(super) depth_indent: f32,
    pub(super) icon_slot_width: f32,
    pub(super) icon_advance: f32,
    pub(super) arrow_reserve: f32,
    pub(super) label_end_padding: f32,
    pub(super) arrow_end_inset: f32,
    pub(super) arrow_slot_width: f32,
    pub(super) divider_inset: f32,
    pub(super) divider_thickness: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DropdownTypographyVisual {
    pub(super) label: f32,
    pub(super) icon: f32,
    pub(super) arrow: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DropdownChromeVisual {
    pub(super) border_width: f32,
    pub(super) focus_width: f32,
    pub(super) shadow_expand: f32,
    pub(super) overlay_z: i32,
    radius: DropdownRadiusRole,
    shadow: DropdownShadowRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DropdownIconsVisual {
    pub(super) expanded: &'static str,
    pub(super) collapsed: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DropdownPaletteVisual {
    primary: ColorValue,
    white: ColorValue,
    primary_active: ColorValue,
    elevated_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_quaternary: ColorValue,
    fill_tertiary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DropdownRadiusRole {
    Small,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DropdownShadowRole {
    Secondary,
}

impl DropdownRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

impl DropdownShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Secondary => tokens.box_shadow_secondary(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct DropdownVisual {
    pub(super) layout: DropdownLayoutVisual,
    pub(super) typography: DropdownTypographyVisual,
    pub(super) chrome: DropdownChromeVisual,
    pub(super) icons: DropdownIconsVisual,
    palette: DropdownPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/dropdown/dropdown.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedDropdownVisual {
    pub(super) primary: Color,
    pub(super) white: Color,
    pub(super) primary_active: Color,
    pub(super) elevated_background: Color,
    pub(super) border: Color,
    pub(super) text: Color,
    pub(super) text_quaternary: Color,
    pub(super) fill_tertiary: Color,
    pub(super) radius: f32,
    pub(super) shadow: ShadowToken,
}

impl DropdownVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedDropdownVisual {
        ResolvedDropdownVisual {
            primary: self.palette.primary.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            primary_active: self.palette.primary_active.resolve(tokens),
            elevated_background: self.palette.elevated_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            shadow: self.chrome.shadow.resolve(tokens),
        }
    }
}

pub(super) const fn dropdown_radius_small() -> DropdownRadiusRole {
    DropdownRadiusRole::Small
}
pub(super) const fn dropdown_shadow_secondary() -> DropdownShadowRole {
    DropdownShadowRole::Secondary
}
pub(super) const fn dropdown_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn dropdown_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
pub(super) const fn dropdown_primary_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryActive)
}
pub(super) const fn dropdown_elevated_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(super) const fn dropdown_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(super) const fn dropdown_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn dropdown_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(super) const fn dropdown_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
