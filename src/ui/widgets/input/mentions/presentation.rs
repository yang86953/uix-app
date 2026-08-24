//! Mentions 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存输入框、光标、候选行和弹层的全部静态几何与排版。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MentionsLayoutVisual {
    pub(crate) intrinsic_width: f32,
    pub(crate) control_height: f32,
    pub(crate) suggestion_row_height: f32,
    pub(crate) max_popup_height: f32,
    pub(crate) min_popup_width: f32,
    pub(crate) font_size: f32,
    pub(crate) horizontal_padding: f32,
    pub(crate) caret_height: f32,
    pub(crate) caret_width: f32,
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) panel_border_width: f32,
}

// 保存圆角角色与弹层层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MentionsChromeVisual {
    pub(crate) overlay_z: i32,
    radius: MentionsRadiusRole,
}

// 保存 Mentions 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MentionsPaletteVisual {
    background: ColorValue,
    popup_background: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    primary_hover: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_placeholder: ColorValue,
    hover_fill: ColorValue,
}

// 保存 Mentions 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MentionsRadiusRole {
    Small,
}

impl MentionsRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 Mentions 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MentionsVisual {
    pub(crate) layout: MentionsLayoutVisual,
    pub(crate) chrome: MentionsChromeVisual,
    palette: MentionsPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/mentions/mentions.uix");

// 保存输入框与候选弹层同帧共享的主题结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedMentionsVisual {
    pub(crate) background: Color,
    pub(crate) popup_background: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_placeholder: Color,
    pub(crate) hover_fill: Color,
    pub(crate) radius: f32,
}

impl MentionsVisual {
    // 输入框和候选弹层在同一帧只解析一次主题角色。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedMentionsVisual {
        ResolvedMentionsVisual {
            background: self.palette.background.resolve(tokens),
            popup_background: self.palette.popup_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_placeholder: self.palette.text_placeholder.resolve(tokens),
            hover_fill: self.palette.hover_fill.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(crate) const fn mentions_radius_small() -> MentionsRadiusRole {
    MentionsRadiusRole::Small
}
pub(crate) const fn mentions_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn mentions_popup_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn mentions_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn mentions_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn mentions_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
pub(crate) const fn mentions_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn mentions_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn mentions_text_placeholder() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(crate) const fn mentions_hover_fill() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
