//! DatePicker 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::widgets::input::date_calendar::{
    CalendarPanelIconsVisual, CalendarPanelVisual, ResolvedCalendarPanelVisual,
};

// 保存日期输入框的字号、留白、图标槽和描边规格。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DatePickerTriggerVisual {
    pub(crate) font_size: f32,
    pub(crate) horizontal_padding: f32,
    pub(crate) icon_gap: f32,
    pub(crate) icon_slot_width: f32,
    pub(crate) icon_right_inset: f32,
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
}

// 保存组件自然宽度与弹层间距。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DatePickerLayoutVisual {
    pub(crate) intrinsic_width: f32,
    pub(crate) popup_gap: f32,
}

// 保存触发图标、圆角角色和弹层层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DatePickerChromeVisual {
    pub(crate) trigger_icon: &'static str,
    pub(crate) overlay_z: i32,
    trigger_radius: DatePickerRadiusRole,
    popup_radius: DatePickerRadiusRole,
}

// 保存 DatePicker 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DatePickerPaletteVisual {
    primary: ColorValue,
    primary_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    text_tertiary: ColorValue,
    trigger_background: ColorValue,
    popup_background: ColorValue,
    hover_fill: ColorValue,
}

// 保存 DatePicker 使用的主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DatePickerRadiusRole {
    Small,
}

impl DatePickerRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 DatePicker 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DatePickerVisual {
    pub(crate) trigger: DatePickerTriggerVisual,
    pub(crate) layout: DatePickerLayoutVisual,
    pub(crate) calendar: CalendarPanelVisual,
    pub(crate) calendar_icons: CalendarPanelIconsVisual,
    pub(crate) chrome: DatePickerChromeVisual,
    palette: DatePickerPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/date_picker/date_picker.uix");

// 保存同帧一次解析得到的日期输入框与月历主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedDatePickerVisual {
    pub(crate) primary: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) text_tertiary: Color,
    pub(crate) trigger_background: Color,
    pub(crate) trigger_radius: f32,
    calendar: ResolvedCalendarPanelVisual,
}

impl ResolvedDatePickerVisual {
    // 返回共享月历绘制器使用的同帧主题结果。
    pub(crate) const fn calendar(self) -> ResolvedCalendarPanelVisual {
        self.calendar
    }
}

impl DatePickerVisual {
    // 触发框与月历面板在同一帧只解析一次主题角色。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedDatePickerVisual {
        let primary = self.palette.primary.resolve(tokens);
        let border = self.palette.border.resolve(tokens);
        let text = self.palette.text.resolve(tokens);
        let text_secondary = self.palette.text_secondary.resolve(tokens);
        let text_tertiary = self.palette.text_tertiary.resolve(tokens);
        ResolvedDatePickerVisual {
            primary,
            border,
            text,
            text_secondary,
            text_tertiary,
            trigger_background: self.palette.trigger_background.resolve(tokens),
            trigger_radius: self.chrome.trigger_radius.resolve(tokens),
            calendar: ResolvedCalendarPanelVisual {
                primary,
                primary_background: self.palette.primary_background.resolve(tokens),
                border,
                text,
                text_secondary,
                text_tertiary,
                popup_background: self.palette.popup_background.resolve(tokens),
                hover_fill: self.palette.hover_fill.resolve(tokens),
                radius: self.chrome.popup_radius.resolve(tokens),
            },
        }
    }
}

pub(crate) const fn date_picker_radius_small() -> DatePickerRadiusRole {
    DatePickerRadiusRole::Small
}
pub(crate) const fn date_picker_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn date_picker_primary_background() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryBg)
}
pub(crate) const fn date_picker_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn date_picker_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn date_picker_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn date_picker_text_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextTertiary)
}
pub(crate) const fn date_picker_trigger_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn date_picker_popup_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn date_picker_hover_fill() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
