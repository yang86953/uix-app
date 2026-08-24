//! Cascader 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存级联选择器默认固有尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascaderDefaultsVisual {
    pub(crate) intrinsic_width: f32,
}

// 保存触发器、弹层、候选行、图标、光标和加载指示器几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascaderLayoutVisual {
    pub(crate) trigger_height: f32,
    pub(crate) popup_gap: f32,
    pub(crate) popup_column_min_width: f32,
    pub(crate) popup_height: f32,
    pub(crate) item_height: f32,
    pub(crate) wheel_step: f32,
    pub(crate) fallback_popup_sides: f32,
    pub(crate) trigger_horizontal_padding: f32,
    pub(crate) trigger_arrow_gap: f32,
    pub(crate) trailing_slot_width: f32,
    pub(crate) trigger_arrow_right_inset: f32,
    pub(crate) caret_height: f32,
    pub(crate) caret_width: f32,
    pub(crate) item_horizontal_padding: f32,
    pub(crate) column_separator_width: f32,
    pub(crate) loading_radius: f32,
    pub(crate) loading_radius_ratio: f32,
    pub(crate) loading_arc_pi: f32,
    pub(crate) loading_stroke_width: f32,
}

// 保存触发文字、候选文字和空态文字字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascaderTypographyVisual {
    trigger: CascaderFontRole,
    item: CascaderFontRole,
    empty: CascaderFontRole,
}

// 保存进入、退出和加载旋转时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascaderMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) exit_duration: f64,
    pub(crate) loading_cycle: f64,
}

// 保存触发器和子级使用的图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CascaderIconsVisual {
    pub(crate) expanded: &'static str,
    pub(crate) collapsed: &'static str,
    pub(crate) child: &'static str,
}

// 保存输入框、弹层描边、层级与圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascaderChromeVisual {
    pub(crate) normal_border_width: f32,
    pub(crate) focused_border_width: f32,
    pub(crate) popup_border_width: f32,
    pub(crate) overlay_z: i32,
    radius: CascaderRadiusRole,
}

// 保存 Cascader 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CascaderPaletteVisual {
    input_background: ColorValue,
    popup_background: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    primary_background: ColorValue,
    text: ColorValue,
    secondary_text: ColorValue,
    tertiary_text: ColorValue,
}

// 保存主题字体角色或固定字号。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum CascaderFontRole {
    Base,
    Fixed(f32),
}

impl CascaderFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Base => tokens.font_size(),
            Self::Fixed(value) => value,
        }
    }
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CascaderRadiusRole {
    Small,
}

impl CascaderRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 Cascader 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CascaderVisual {
    pub(crate) defaults: CascaderDefaultsVisual,
    pub(crate) layout: CascaderLayoutVisual,
    pub(crate) typography: CascaderTypographyVisual,
    pub(crate) motion: CascaderMotionVisual,
    pub(crate) icons: CascaderIconsVisual,
    pub(crate) chrome: CascaderChromeVisual,
    palette: CascaderPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/cascader/cascader.uix");

// 保存每帧一次解析得到的主题与排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedCascaderVisual {
    pub(crate) input_background: Color,
    pub(crate) popup_background: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) primary_background: Color,
    pub(crate) text: Color,
    pub(crate) secondary_text: Color,
    pub(crate) tertiary_text: Color,
    pub(crate) radius: f32,
    pub(crate) trigger_font_size: f32,
    pub(crate) item_font_size: f32,
    pub(crate) empty_font_size: f32,
}

impl CascaderVisual {
    // 同帧触发器、普通列和搜索结果复用一次主题解析。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedCascaderVisual {
        ResolvedCascaderVisual {
            input_background: self.palette.input_background.resolve(tokens),
            popup_background: self.palette.popup_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            primary_background: self.palette.primary_background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            secondary_text: self.palette.secondary_text.resolve(tokens),
            tertiary_text: self.palette.tertiary_text.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            trigger_font_size: self.typography.trigger.resolve(tokens),
            item_font_size: self.typography.item.resolve(tokens),
            empty_font_size: self.typography.empty.resolve(tokens),
        }
    }
}

pub(crate) const fn cascader_font_base() -> CascaderFontRole {
    CascaderFontRole::Base
}
pub(crate) const fn cascader_font_fixed(value: f32) -> CascaderFontRole {
    CascaderFontRole::Fixed(value)
}
pub(crate) const fn cascader_radius_small() -> CascaderRadiusRole {
    CascaderRadiusRole::Small
}
pub(crate) const fn cascader_input_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn cascader_popup_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn cascader_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn cascader_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn cascader_primary_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(crate) const fn cascader_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn cascader_secondary_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn cascader_tertiary_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}
