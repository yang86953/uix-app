//! Pagination 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PaginationLayoutVisual {
    pub(super) default_item_size: f32,
    pub(super) gap: f32,
    pub(super) extra_gap: f32,
    pub(super) total_width: f32,
    pub(super) size_changer_width: f32,
    pub(super) hit_vertical_extra: f32,
    pub(super) simple_width_factor: f32,
    pub(super) jumper_width: f32,
    pub(super) jumper_label_width: f32,
    pub(super) jumper_input_width: f32,
    pub(super) jumper_suffix_gap: f32,
    pub(super) jumper_suffix_width: f32,
    pub(super) cursor_horizontal_padding: f32,
    pub(super) cursor_vertical_inset: f32,
    pub(super) cursor_width: f32,
    pub(super) changer_text_start: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PaginationTypographyVisual {
    small: PaginationFontRole,
    pub(super) page: f32,
    pub(super) icon: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PaginationChromeVisual {
    pub(super) border_width: f32,
    pub(super) focus_width: f32,
    radius: PaginationRadiusRole,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PaginationDefaultsVisual {
    pub(super) show_size_changer: bool,
    pub(super) show_total: bool,
    pub(super) simple: bool,
    pub(super) show_jumper: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PaginationIconsVisual {
    pub(super) previous: &'static str,
    pub(super) next: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PaginationPaletteVisual {
    primary: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    white: ColorValue,
    container_background: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaginationFontRole {
    Small,
}

impl PaginationFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.font_size_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaginationRadiusRole {
    Small,
}

impl PaginationRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PaginationVisual {
    pub(super) layout: PaginationLayoutVisual,
    pub(super) typography: PaginationTypographyVisual,
    pub(super) chrome: PaginationChromeVisual,
    pub(super) defaults: PaginationDefaultsVisual,
    pub(super) icons: PaginationIconsVisual,
    palette: PaginationPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/pagination/pagination.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedPaginationVisual {
    pub(super) primary: Color,
    pub(super) border: Color,
    pub(super) text: Color,
    pub(super) text_secondary: Color,
    pub(super) white: Color,
    pub(super) container_background: Color,
    pub(super) small_font_size: f32,
    pub(super) radius: f32,
}

impl PaginationVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedPaginationVisual {
        ResolvedPaginationVisual {
            primary: self.palette.primary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            container_background: self.palette.container_background.resolve(tokens),
            small_font_size: self.typography.small.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(super) const fn pagination_font_small() -> PaginationFontRole {
    PaginationFontRole::Small
}
pub(super) const fn pagination_radius_small() -> PaginationRadiusRole {
    PaginationRadiusRole::Small
}
pub(super) const fn pagination_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn pagination_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(super) const fn pagination_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn pagination_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn pagination_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
pub(super) const fn pagination_container_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
