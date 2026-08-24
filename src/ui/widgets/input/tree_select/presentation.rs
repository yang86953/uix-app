//! TreeSelect 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存触发器、弹层与层级行的静态几何和排版。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TreeSelectLayoutVisual {
    pub(crate) intrinsic_width: f32,
    pub(crate) trigger_height: f32,
    pub(crate) row_height: f32,
    pub(crate) max_dropdown_height: f32,
    pub(crate) min_dropdown_width: f32,
    pub(crate) font_size: f32,
    pub(crate) left_padding: f32,
    pub(crate) arrow_slot: f32,
    pub(crate) arrow_icon_size: f32,
    pub(crate) row_horizontal_padding: f32,
    pub(crate) indent_width: f32,
    pub(crate) indent_base: f32,
    pub(crate) indent_right_reserve: f32,
    pub(crate) text_right_padding: f32,
}

// 保存描边、圆角与浮层层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TreeSelectChromeVisual {
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) overlay_z: i32,
    radius: TreeSelectRadiusRole,
}

// 保存 TreeSelect 使用的图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TreeSelectIconsVisual {
    pub(crate) arrow_up: &'static str,
    pub(crate) arrow_down: &'static str,
}

// 保存 TreeSelect 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TreeSelectPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    primary_background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_quaternary: ColorValue,
    fill_tertiary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeSelectRadiusRole {
    Small,
}

impl TreeSelectRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TreeSelectVisual {
    pub(crate) layout: TreeSelectLayoutVisual,
    pub(crate) chrome: TreeSelectChromeVisual,
    pub(crate) icons: TreeSelectIconsVisual,
    palette: TreeSelectPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/tree_select/tree_select.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTreeSelectVisual {
    pub(crate) background: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) primary_background: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) radius: f32,
}

impl TreeSelectVisual {
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedTreeSelectVisual {
        ResolvedTreeSelectVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            primary_background: self.palette.primary_background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
        }
    }
}

pub(crate) const fn tree_select_radius_small() -> TreeSelectRadiusRole {
    TreeSelectRadiusRole::Small
}
pub(crate) const fn tree_select_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn tree_select_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn tree_select_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn tree_select_primary_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(crate) const fn tree_select_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn tree_select_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn tree_select_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(crate) const fn tree_select_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
