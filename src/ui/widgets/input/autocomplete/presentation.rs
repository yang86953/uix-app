//! AutoComplete 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存输入框默认尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoCompleteDefaultsVisual {
    pub(crate) intrinsic_width: f32,
}

// 保存输入框、光标、候选行和弹层使用的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoCompleteLayoutVisual {
    pub(crate) control_height: f32,
    pub(crate) row_height: f32,
    pub(crate) max_popup_height: f32,
    pub(crate) min_popup_width: f32,
    pub(crate) input_horizontal_padding: f32,
    pub(crate) option_horizontal_padding: f32,
    pub(crate) caret_height: f32,
    pub(crate) caret_width: f32,
    pub(crate) fallback_popup_sides: f32,
}

// 保存输入文字、占位文字和候选行使用的排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoCompleteTypographyVisual {
    pub(crate) font_size: f32,
}

// 保存候选弹层进入与退出时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoCompleteMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) exit_duration: f64,
}

// 保存输入框、弹层描边、层级与圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoCompleteChromeVisual {
    pub(crate) normal_border_width: f32,
    pub(crate) focused_border_width: f32,
    pub(crate) popup_border_width: f32,
    pub(crate) overlay_z: i32,
    radius: AutoCompleteRadiusRole,
}

// 保存 AutoComplete 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AutoCompletePaletteVisual {
    input_background: ColorValue,
    popup_background: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    primary_hover: ColorValue,
    text: ColorValue,
    secondary_text: ColorValue,
    placeholder: ColorValue,
    option_hover: ColorValue,
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AutoCompleteRadiusRole {
    Small,
}

impl AutoCompleteRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 AutoComplete 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AutoCompleteVisual {
    pub(crate) defaults: AutoCompleteDefaultsVisual,
    pub(crate) layout: AutoCompleteLayoutVisual,
    pub(crate) typography: AutoCompleteTypographyVisual,
    pub(crate) motion: AutoCompleteMotionVisual,
    pub(crate) chrome: AutoCompleteChromeVisual,
    palette: AutoCompletePaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/autocomplete/autocomplete.uix");

// 保存每帧一次解析得到的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedAutoCompleteVisual {
    pub(crate) input_background: Color,
    pub(crate) popup_background: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) text: Color,
    pub(crate) secondary_text: Color,
    pub(crate) placeholder: Color,
    pub(crate) option_hover: Color,
    pub(crate) radius: f32,
}

impl AutoCompleteVisual {
    // 同帧输入框与弹层复用一次主题角色解析。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedAutoCompleteVisual {
        ResolvedAutoCompleteVisual {
            input_background: self.palette.input_background.resolve(tokens),
            popup_background: self.palette.popup_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            secondary_text: self.palette.secondary_text.resolve(tokens),
            placeholder: self.palette.placeholder.resolve(tokens),
            option_hover: self.palette.option_hover.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(crate) const fn autocomplete_radius_small() -> AutoCompleteRadiusRole {
    AutoCompleteRadiusRole::Small
}

pub(crate) const fn autocomplete_input_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}

pub(crate) const fn autocomplete_popup_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

pub(crate) const fn autocomplete_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}

pub(crate) const fn autocomplete_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

pub(crate) const fn autocomplete_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}

pub(crate) const fn autocomplete_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}

pub(crate) const fn autocomplete_secondary_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}

pub(crate) const fn autocomplete_placeholder() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}

pub(crate) const fn autocomplete_option_hover() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
