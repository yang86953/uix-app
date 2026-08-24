//! Steps 的 UIX 静态视觉契约与主题解析。

use super::StepsDirection;
use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StepsLayoutVisual {
    pub(super) step_extent: f32,
    pub(super) horizontal_step_max_width: f32,
    pub(super) horizontal_width: f32,
    pub(super) horizontal_height: f32,
    pub(super) vertical_width: f32,
    pub(super) circle_radius: f32,
    pub(super) center_offset: f32,
    pub(super) line_half_width: f32,
    pub(super) line_thickness: f32,
    pub(super) dot_radius_factor: f32,
    pub(super) title_gap: f32,
    pub(super) title_height: f32,
    pub(super) description_offset: f32,
    pub(super) description_height: f32,
    pub(super) vertical_text_start: f32,
    pub(super) vertical_title_offset: f32,
    pub(super) vertical_text_end_reserve: f32,
    pub(super) vertical_title_height: f32,
    pub(super) vertical_description_offset: f32,
    pub(super) vertical_description_height: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StepsTypographyVisual {
    marker: StepsFontRole,
    pub(super) title: f32,
    pub(super) description: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StepsChromeVisual {
    pub(super) marker_border_width: f32,
    pub(super) focus_width: f32,
    radius: StepsRadiusRole,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StepsDefaultsVisual {
    pub(super) direction: StepsDirection,
    pub(super) clickable: bool,
    pub(super) dot: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StepsIconsVisual {
    pub(super) finish: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct StepsPaletteVisual {
    primary: ColorValue,
    error: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    fill: ColorValue,
    white: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StepsFontRole {
    Base,
}

impl StepsFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Base => tokens.font_size(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum StepsRadiusRole {
    Small,
}

impl StepsRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct StepsVisual {
    pub(super) layout: StepsLayoutVisual,
    pub(super) typography: StepsTypographyVisual,
    pub(super) chrome: StepsChromeVisual,
    pub(super) defaults: StepsDefaultsVisual,
    pub(super) icons: StepsIconsVisual,
    palette: StepsPaletteVisual,
}

crate::uix_items!("src/ui/widgets/navigation/steps/steps.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ResolvedStepsVisual {
    pub(super) primary: Color,
    pub(super) error: Color,
    pub(super) text: Color,
    pub(super) text_secondary: Color,
    pub(super) fill: Color,
    pub(super) white: Color,
    pub(super) marker_font_size: f32,
    pub(super) radius: f32,
}

impl StepsVisual {
    pub(super) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedStepsVisual {
        ResolvedStepsVisual {
            primary: self.palette.primary.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            fill: self.palette.fill.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            marker_font_size: self.typography.marker.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(super) const fn steps_direction_horizontal() -> StepsDirection {
    StepsDirection::Horizontal
}
pub(super) const fn steps_font_base() -> StepsFontRole {
    StepsFontRole::Base
}
pub(super) const fn steps_radius_small() -> StepsRadiusRole {
    StepsRadiusRole::Small
}
pub(super) const fn steps_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(super) const fn steps_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(super) const fn steps_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(super) const fn steps_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(super) const fn steps_fill() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Fill)
}
pub(super) const fn steps_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
