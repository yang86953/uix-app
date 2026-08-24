//! Breadcrumb 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BreadcrumbLayoutVisual {
    pub(super) height: f32,
    pub(super) title_glyph_width: f32,
    pub(super) separator_glyph_width: f32,
    pub(super) icon_slot_width: f32,
    pub(super) icon_text_gap: f32,
    pub(super) overflow_row_height: f32,
    pub(super) overflow_min_width: f32,
    pub(super) overflow_horizontal_padding: f32,
    pub(super) text_horizontal_padding: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BreadcrumbTypographyVisual {
    pub(super) title: f32,
    pub(super) separator: f32,
    pub(super) icon: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BreadcrumbChromeVisual {
    pub(super) border_width: f32,
    pub(super) focus_width: f32,
    radius: BreadcrumbRadiusRole,
    shadow: BreadcrumbShadowRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct BreadcrumbPaletteVisual {
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    fill_tertiary: ColorValue,
    elevated_background: ColorValue,
    border: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BreadcrumbRadiusRole {
    Small,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum BreadcrumbShadowRole {
    Secondary,
}

impl BreadcrumbRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}
impl BreadcrumbShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Secondary => tokens.box_shadow_secondary(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct BreadcrumbVisual {
    pub(super) layout: BreadcrumbLayoutVisual,
    pub(super) typography: BreadcrumbTypographyVisual,
    pub(super) chrome: BreadcrumbChromeVisual,
    palette: BreadcrumbPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/breadcrumb/breadcrumb.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedBreadcrumbVisual {
    pub(super) text: Color,
    pub(super) text_secondary: Color,
    pub(super) primary: Color,
    pub(super) fill_tertiary: Color,
    pub(super) elevated_background: Color,
    pub(super) border: Color,
    pub(super) radius: f32,
    pub(super) shadow: ShadowToken,
}

impl BreadcrumbVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedBreadcrumbVisual {
        ResolvedBreadcrumbVisual {
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            elevated_background: self.palette.elevated_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            shadow: self.chrome.shadow.resolve(tokens),
        }
    }
}

pub(super) const fn breadcrumb_radius_small() -> BreadcrumbRadiusRole {
    BreadcrumbRadiusRole::Small
}
pub(super) const fn breadcrumb_shadow_secondary() -> BreadcrumbShadowRole {
    BreadcrumbShadowRole::Secondary
}
pub(super) const fn breadcrumb_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn breadcrumb_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn breadcrumb_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn breadcrumb_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(super) const fn breadcrumb_elevated_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(super) const fn breadcrumb_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
