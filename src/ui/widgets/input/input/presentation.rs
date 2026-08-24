//! Input 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::platform::windowing::ControlSize;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存三档控件高度、自然尺寸、文本区、附件槽和编辑标记几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputLayoutVisual {
    pub(crate) small_height: f32,
    pub(crate) medium_height: f32,
    pub(crate) large_height: f32,
    pub(crate) natural_width: f32,
    pub(crate) textarea_min_height: f32,
    pub(crate) textarea_intrinsic_vertical_padding: f32,
    pub(crate) textarea_top_padding: f32,
    pub(crate) textarea_content_vertical_inset: f32,
    pub(crate) textarea_content_min_width: f32,
    pub(crate) textarea_content_min_height: f32,
    pub(crate) horizontal_padding: f32,
    pub(crate) singleline_text_min_width: f32,
    pub(crate) prefix_width: f32,
    pub(crate) suffix_width: f32,
    pub(crate) clear_width: f32,
    pub(crate) password_width: f32,
    pub(crate) search_width: f32,
    pub(crate) addon_horizontal_padding: f32,
    pub(crate) addon_text_inset: f32,
    pub(crate) prefix_icon_inset: f32,
    pub(crate) caret_width: f32,
    pub(crate) caret_vertical_inset: f32,
    pub(crate) caret_min_font_ratio: f32,
    pub(crate) composition_bottom_inset_textarea: f32,
    pub(crate) composition_bottom_inset_singleline: f32,
    pub(crate) composition_min_width: f32,
    pub(crate) composition_underline_height: f32,
    pub(crate) scroll_right_margin: f32,
    pub(crate) scroll_end_guard: f32,
}

impl InputLayoutVisual {
    // 返回当前控件尺寸档位的 UIX 高度。
    pub(crate) const fn control_height(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_height,
            ControlSize::Medium => self.medium_height,
            ControlSize::Large => self.large_height,
        }
    }
}

// 保存正文、行高、附件、状态和图标字号。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputTypographyVisual {
    pub(crate) font_size: f32,
    pub(crate) line_height: f32,
    pub(crate) addon_font_size: f32,
    pub(crate) status_font_size: f32,
    pub(crate) prefix_icon_size: f32,
    pub(crate) password_icon_size: f32,
    pub(crate) layout_line_height_ratio: f32,
    pub(crate) fallback_line_height_ratio: f32,
    accessory_icon_size: InputFontSizeRole,
}

// 保存描边、选区透明度、状态消息高度和圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputChromeVisual {
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) selection_alpha: u32,
    pub(crate) status_message_height: f32,
    radius: InputRadiusRole,
}

// 保存清除、密码可见性与搜索图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputIconsVisual {
    pub(crate) clear: &'static str,
    pub(crate) password_visible: &'static str,
    pub(crate) password_hidden: &'static str,
    pub(crate) search: &'static str,
}

// 保存 Input 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct InputPaletteVisual {
    primary: ColorValue,
    primary_hover: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_tertiary: ColorValue,
    text_quaternary: ColorValue,
    fill_tertiary: ColorValue,
    background: ColorValue,
    background_elevated: ColorValue,
    success: ColorValue,
    warning: ColorValue,
    error: ColorValue,
}

// 保存 Input 使用的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputFontSizeRole {
    Small,
}

impl InputFontSizeRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.font_size_sm(),
        }
    }
}

// 保存 Input 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InputRadiusRole {
    Small,
}

impl InputRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 Input 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct InputVisual {
    pub(crate) layout: InputLayoutVisual,
    pub(crate) typography: InputTypographyVisual,
    pub(crate) chrome: InputChromeVisual,
    pub(crate) icons: InputIconsVisual,
    palette: InputPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/input/input.uix");

// 保存一帧内单行、多行和状态消息共享的主题结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedInputVisual {
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_tertiary: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) background: Color,
    pub(crate) background_elevated: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) radius: f32,
    pub(crate) accessory_icon_size: f32,
}

impl InputVisual {
    // 单行、多行和状态消息在同一帧只解析一次主题角色。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedInputVisual {
        ResolvedInputVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_tertiary: self.palette.text_tertiary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            background: self.palette.background.resolve(tokens),
            background_elevated: self.palette.background_elevated.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            warning: self.palette.warning.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            accessory_icon_size: self.typography.accessory_icon_size.resolve(tokens),
        }
    }
}

pub(crate) const fn input_font_size_small() -> InputFontSizeRole {
    InputFontSizeRole::Small
}
pub(crate) const fn input_radius_small() -> InputRadiusRole {
    InputRadiusRole::Small
}
pub(crate) const fn input_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn input_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
pub(crate) const fn input_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn input_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn input_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn input_text_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}
pub(crate) const fn input_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(crate) const fn input_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn input_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn input_background_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn input_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
pub(crate) const fn input_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn input_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
