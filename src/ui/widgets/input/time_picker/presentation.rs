//! TimePicker 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::platform::windowing::ControlSize;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存触发器、图标、时间面板和双列列表的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimePickerLayoutVisual {
    pub(crate) small_height: f32,
    pub(crate) medium_height: f32,
    pub(crate) large_height: f32,
    pub(crate) natural_width: f32,
    pub(crate) popup_gap: f32,
    pub(crate) popup_height: f32,
    pub(crate) popup_min_width: f32,
    pub(crate) item_height: f32,
    pub(crate) horizontal_padding: f32,
    pub(crate) icon_gap: f32,
    pub(crate) icon_slot_width: f32,
    pub(crate) icon_right_inset: f32,
    pub(crate) column_ratio: f32,
}

impl TimePickerLayoutVisual {
    // 返回 UIX 为当前尺寸档位声明的控件高度。
    pub(crate) const fn control_height(self, size: ControlSize) -> f32 {
        match size {
            ControlSize::Small => self.small_height,
            ControlSize::Medium => self.medium_height,
            ControlSize::Large => self.large_height,
        }
    }
}

// 保存触发器文字、列文字和图标的主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimePickerTypographyVisual {
    trigger: TimePickerFontRole,
    item: TimePickerFontRole,
    icon: TimePickerFontRole,
}

// 保存描边、分隔线、圆角和浮层层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimePickerChromeVisual {
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) divider_width: f32,
    pub(crate) overlay_z: i32,
    radius: TimePickerRadiusRole,
}

// 保存 TimePicker 使用的图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimePickerIconsVisual {
    pub(crate) clock: &'static str,
}

// 保存 TimePicker 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TimePickerPaletteVisual {
    primary: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_tertiary: ColorValue,
    background: ColorValue,
    popup_background: ColorValue,
    primary_background: ColorValue,
}

// 保存主题字号角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimePickerFontRole {
    Normal,
}

impl TimePickerFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Normal => tokens.font_size(),
        }
    }
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TimePickerRadiusRole {
    Small,
}

impl TimePickerRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 TimePicker 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TimePickerVisual {
    pub(crate) layout: TimePickerLayoutVisual,
    typography: TimePickerTypographyVisual,
    pub(crate) chrome: TimePickerChromeVisual,
    pub(crate) icons: TimePickerIconsVisual,
    palette: TimePickerPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/time_picker/time_picker.uix");

// 保存触发器与面板同帧共享的主题解析结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTimePickerVisual {
    pub(crate) primary: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_tertiary: Color,
    pub(crate) background: Color,
    pub(crate) popup_background: Color,
    pub(crate) primary_background: Color,
    pub(crate) radius: f32,
    pub(crate) trigger_font_size: f32,
    pub(crate) item_font_size: f32,
    pub(crate) icon_size: f32,
}

impl TimePickerVisual {
    // 触发器与面板在同一帧只解析一次主题角色。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedTimePickerVisual {
        ResolvedTimePickerVisual {
            primary: self.palette.primary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            text_tertiary: self.palette.text_tertiary.resolve(tokens),
            background: self.palette.background.resolve(tokens),
            popup_background: self.palette.popup_background.resolve(tokens),
            primary_background: self.palette.primary_background.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            trigger_font_size: self.typography.trigger.resolve(tokens),
            item_font_size: self.typography.item.resolve(tokens),
            icon_size: self.typography.icon.resolve(tokens),
        }
    }
}

pub(crate) const fn time_picker_font_normal() -> TimePickerFontRole {
    TimePickerFontRole::Normal
}
pub(crate) const fn time_picker_radius_small() -> TimePickerRadiusRole {
    TimePickerRadiusRole::Small
}
pub(crate) const fn time_picker_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn time_picker_primary_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(crate) const fn time_picker_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn time_picker_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn time_picker_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn time_picker_text_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}
pub(crate) const fn time_picker_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn time_picker_popup_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
