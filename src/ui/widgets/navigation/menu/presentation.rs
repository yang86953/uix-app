//! Menu 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuLayoutVisual {
    pub(super) default_item_height: f32,
    pub(super) horizontal_min_width: f32,
    pub(super) expanded_width: f32,
    pub(super) compact_width: f32,
    pub(super) glyph_width: f32,
    pub(super) horizontal_padding: f32,
    pub(super) depth_indent: f32,
    pub(super) icon_extra_width: f32,
    pub(super) active_inset: f32,
    pub(super) active_thickness: f32,
    pub(super) icon_text_gap: f32,
    pub(super) icon_slot_width: f32,
    pub(super) icon_advance: f32,
    pub(super) plain_label_padding: f32,
    pub(super) icon_label_padding: f32,
    pub(super) vertical_icon_start: f32,
    pub(super) label_end_padding: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuTypographyVisual {
    label: MenuFontRole,
    compact_fallback: MenuFontRole,
    pub(super) icon: f32,
    pub(super) icon_label: f32,
    pub(super) compact_icon: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuChromeVisual {
    pub(super) focus_width: f32,
    radius: MenuRadiusRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MenuPaletteVisual {
    primary: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    fill_tertiary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuFontRole {
    Base,
    Large,
}

impl MenuFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Base => tokens.font_size(),
            Self::Large => tokens.font_size_lg(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MenuRadiusRole {
    Small,
}

impl MenuRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct MenuVisual {
    pub(super) layout: MenuLayoutVisual,
    pub(super) typography: MenuTypographyVisual,
    pub(super) chrome: MenuChromeVisual,
    palette: MenuPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/menu/menu.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedMenuVisual {
    pub(super) primary: Color,
    pub(super) text: Color,
    pub(super) text_secondary: Color,
    pub(super) fill_tertiary: Color,
    pub(super) label_font_size: f32,
    pub(super) compact_fallback_font_size: f32,
    pub(super) radius: f32,
}

impl MenuVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedMenuVisual {
        ResolvedMenuVisual {
            primary: self.palette.primary.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            label_font_size: self.typography.label.resolve(tokens),
            compact_fallback_font_size: self.typography.compact_fallback.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(super) const fn menu_font_base() -> MenuFontRole {
    MenuFontRole::Base
}
pub(super) const fn menu_font_large() -> MenuFontRole {
    MenuFontRole::Large
}
pub(super) const fn menu_radius_small() -> MenuRadiusRole {
    MenuRadiusRole::Small
}
pub(super) const fn menu_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn menu_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn menu_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn menu_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
