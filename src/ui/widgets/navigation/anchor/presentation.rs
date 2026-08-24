//! Anchor 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AnchorLayoutVisual {
    pub(super) row_height: f32,
    pub(super) indicator_width: f32,
    pub(super) label_x: f32,
    pub(super) label_char_width: f32,
    pub(super) label_horizontal_space: f32,
    pub(super) navigation_min_width: f32,
    pub(super) container_width: f32,
    pub(super) container_min_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AnchorDefaultsVisual {
    pub(super) show_ink: bool,
    pub(super) bounds: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AnchorChromeVisual {
    pub(super) divider_width: f32,
    pub(super) border_width: f32,
    pub(super) focus_width: f32,
    radius: AnchorRadiusRole,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct AnchorPaletteVisual {
    primary: ColorValue,
    text_secondary: ColorValue,
    border_secondary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AnchorRadiusRole {
    Small,
}

impl AnchorRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct AnchorVisual {
    pub(super) layout: AnchorLayoutVisual,
    pub(super) defaults: AnchorDefaultsVisual,
    pub(super) chrome: AnchorChromeVisual,
    palette: AnchorPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/anchor/anchor.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedAnchorVisual {
    pub(super) primary: Color,
    pub(super) text_secondary: Color,
    pub(super) border_secondary: Color,
    pub(super) font_size: f32,
    pub(super) radius: f32,
}

impl AnchorVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedAnchorVisual {
        ResolvedAnchorVisual {
            primary: self.palette.primary.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            border_secondary: self.palette.border_secondary.resolve(tokens),
            font_size: tokens.font_size(),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(super) const fn anchor_radius_small() -> AnchorRadiusRole {
    AnchorRadiusRole::Small
}
pub(super) const fn anchor_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn anchor_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn anchor_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
