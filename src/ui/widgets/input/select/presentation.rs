//! Select 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::platform::windowing::ControlSize;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};

// 保存控件、弹层、标签、候选行与加载器的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectLayoutVisual {
    pub(crate) small_height: f32,
    pub(crate) medium_height: f32,
    pub(crate) large_height: f32,
    pub(crate) natural_min_width: f32,
    pub(crate) intrinsic_extra_width: f32,
    pub(crate) row_height: f32,
    pub(crate) max_dropdown_height: f32,
    pub(crate) control_left_padding: f32,
    pub(crate) arrow_slot_width: f32,
    pub(crate) caret_vertical_inset: f32,
    pub(crate) caret_width: f32,
    pub(crate) tag_vertical_inset: f32,
    pub(crate) tag_max_height: f32,
    pub(crate) tag_min_remaining: f32,
    pub(crate) tag_close_width: f32,
    pub(crate) tag_horizontal_padding: f32,
    pub(crate) tag_gap: f32,
    pub(crate) row_horizontal_padding: f32,
    pub(crate) custom_multi_left: f32,
    pub(crate) custom_right_inset: f32,
    pub(crate) option_right_inset: f32,
    pub(crate) check_size: f32,
    pub(crate) selected_icon_slot: f32,
    pub(crate) selected_icon_width: f32,
    pub(crate) loading_radius: f32,
    pub(crate) loading_radius_ratio: f32,
}

impl SelectLayoutVisual {
    // 返回 UIX 为当前尺寸档位声明的控件高度。
    pub(crate) const fn control_height(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_height,
            ControlSize::Medium => self.medium_height,
            ControlSize::Large => self.large_height,
        }
    }
}

// 保存控件、候选、标签与图标的字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectTypographyVisual {
    text: SelectFontRole,
    group: SelectFontRole,
    tag: SelectFontRole,
    arrow_icon: SelectFontRole,
    tag_close_icon: SelectFontRole,
    option_check_icon: SelectFontRole,
    selected_check_icon: SelectFontRole,
}

// 保存描边、圆角、阴影、浮层与加载器静态装饰。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectChromeVisual {
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) tag_radius: f32,
    pub(crate) check_radius: f32,
    pub(crate) shadow_expand: f32,
    pub(crate) loading_stroke: f32,
    pub(crate) loading_sweep_pi: f32,
    pub(crate) overlay_z: i32,
    radius: SelectRadiusRole,
    shadow: SelectShadowRole,
}

// 保存加载器的静态循环节奏。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectMotionVisual {
    pub(crate) loading_cycle_seconds: f32,
}

// 保存 Select 使用的图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectIconsVisual {
    pub(crate) arrow_up: &'static str,
    pub(crate) arrow_down: &'static str,
    pub(crate) close: &'static str,
    pub(crate) check: &'static str,
}

// 保存 Select 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectPaletteVisual {
    primary: ColorValue,
    primary_background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_quaternary: ColorValue,
    fill_tertiary: ColorValue,
    fill_quaternary: ColorValue,
    background: ColorValue,
    background_elevated: ColorValue,
    border: ColorValue,
    border_secondary: ColorValue,
    white: ColorValue,
}

// 保存固定字号或主题小字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum SelectFontRole {
    Small,
    Fixed(f32),
}

impl SelectFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.font_size_sm(),
            Self::Fixed(value) => value,
        }
    }

    // 无绘制上下文的固有尺寸估算沿用角色的稳定基准字号。
    const fn estimate(self) -> f32 {
        match self {
            Self::Small => 12.0,
            Self::Fixed(value) => value,
        }
    }
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectRadiusRole {
    Small,
}

impl SelectRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存主题阴影角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SelectShadowRole {
    Secondary,
}

impl SelectShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Secondary => tokens.box_shadow_secondary(),
        }
    }
}

// 全部 Select 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct SelectVisual {
    pub(crate) layout: SelectLayoutVisual,
    typography: SelectTypographyVisual,
    pub(crate) chrome: SelectChromeVisual,
    pub(crate) motion: SelectMotionVisual,
    pub(crate) icons: SelectIconsVisual,
    palette: SelectPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/select/select.uix");

// 保存一帧内控件与弹层共享的主题解析结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedSelectVisual {
    pub(crate) primary: Color,
    pub(crate) primary_background: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) fill_quaternary: Color,
    pub(crate) background: Color,
    pub(crate) background_elevated: Color,
    pub(crate) border: Color,
    pub(crate) border_secondary: Color,
    pub(crate) white: Color,
    pub(crate) radius: f32,
    pub(crate) shadow: ShadowToken,
    pub(crate) text_size: f32,
    pub(crate) group_text_size: f32,
    pub(crate) tag_text_size: f32,
    pub(crate) arrow_icon_size: f32,
    pub(crate) tag_close_icon_size: f32,
    pub(crate) option_check_icon_size: f32,
    pub(crate) selected_check_icon_size: f32,
}

impl SelectVisual {
    // 返回固有宽度估算使用的正文基准字号。
    pub(crate) const fn intrinsic_text_size(&self) -> f32 {
        self.typography.text.estimate()
    }

    // 返回固有宽度估算使用的分组基准字号。
    pub(crate) const fn intrinsic_group_text_size(&self) -> f32 {
        self.typography.group.estimate()
    }

    // 控件与弹层在同一帧只解析一次主题角色。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedSelectVisual {
        ResolvedSelectVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_background: self.palette.primary_background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            fill_quaternary: self.palette.fill_quaternary.resolve(tokens),
            background: self.palette.background.resolve(tokens),
            background_elevated: self.palette.background_elevated.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            border_secondary: self.palette.border_secondary.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            shadow: self.chrome.shadow.resolve(tokens),
            text_size: self.typography.text.resolve(tokens),
            group_text_size: self.typography.group.resolve(tokens),
            tag_text_size: self.typography.tag.resolve(tokens),
            arrow_icon_size: self.typography.arrow_icon.resolve(tokens),
            tag_close_icon_size: self.typography.tag_close_icon.resolve(tokens),
            option_check_icon_size: self.typography.option_check_icon.resolve(tokens),
            selected_check_icon_size: self.typography.selected_check_icon.resolve(tokens),
        }
    }
}

pub(crate) const fn select_font_small() -> SelectFontRole {
    SelectFontRole::Small
}
pub(crate) const fn select_font_fixed(value: f32) -> SelectFontRole {
    SelectFontRole::Fixed(value)
}
pub(crate) const fn select_radius_small() -> SelectRadiusRole {
    SelectRadiusRole::Small
}
pub(crate) const fn select_shadow_secondary() -> SelectShadowRole {
    SelectShadowRole::Secondary
}
pub(crate) const fn select_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn select_primary_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(crate) const fn select_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn select_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn select_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(crate) const fn select_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn select_fill_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillQuaternary)
}
pub(crate) const fn select_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn select_background_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn select_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn select_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
pub(crate) const fn select_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
