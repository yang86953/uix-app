//! Alert 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

use crate::draw::Color;
use crate::platform::capabilities::StatusLevel;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存 UIX 声明的固有尺寸与可由 Rust 调用方覆盖的初始视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AlertDefaultsVisual {
    pub(crate) width: f32,
    pub(crate) base_height: f32,
    pub(crate) description_height: f32,
    pub(crate) show_icon: bool,
    pub(crate) banner: bool,
}

// 保存关闭区、操作区、图标、正文和强调条的静态布局。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AlertLayoutVisual {
    pub(crate) close_width: f32,
    pub(crate) close_action_gap: f32,
    pub(crate) content_right_gap: f32,
    pub(crate) action_width: f32,
    pub(crate) action_content_gap: f32,
    pub(crate) icon_max_width: f32,
    pub(crate) icon_gap: f32,
    pub(crate) no_icon_left: f32,
    pub(crate) content_vertical_inset: f32,
    pub(crate) content_vertical_ratio: f32,
    pub(crate) message_height_ratio: f32,
    pub(crate) accent_x: f32,
    pub(crate) accent_y: f32,
    pub(crate) accent_width: f32,
    pub(crate) icon_x: f32,
}

// 保存强调条、操作/关闭反馈与键盘焦点的静态装饰。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AlertChromeVisual {
    pub(crate) accent_radius: f32,
    pub(crate) action_inset: f32,
    pub(crate) close_inset: f32,
    pub(crate) focus_stroke: f32,
    container_radius: AlertRadiusRole,
    interaction_radius: AlertRadiusRole,
}

// 保存正文、说明、操作和图标排版与图标名称。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AlertTypographyVisual {
    pub(crate) message: f32,
    pub(crate) description: f32,
    pub(crate) action: f32,
    pub(crate) status_icon: f32,
    pub(crate) close_icon: f32,
    pub(crate) success_icon: &'static str,
    pub(crate) info_icon: &'static str,
    pub(crate) warning_icon: &'static str,
    pub(crate) error_icon: &'static str,
    pub(crate) close_icon_name: &'static str,
}

// 保存单个状态的背景与前景主题角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlertStatusPaletteVisual {
    background: ColorValue,
    foreground: ColorValue,
}

// 保存四种状态和交互/正文使用的主题角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AlertPaletteVisual {
    statuses: [AlertStatusPaletteVisual; 4],
    text: ColorValue,
    text_secondary: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    primary: ColorValue,
}

// 保存圆角令牌的静态语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AlertRadiusRole {
    Normal,
    Small,
}

impl AlertRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Normal => tokens.border_radius(),
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 Alert 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AlertVisual {
    pub(crate) defaults: AlertDefaultsVisual,
    pub(crate) layout: AlertLayoutVisual,
    pub(crate) chrome: AlertChromeVisual,
    pub(crate) typography: AlertTypographyVisual,
    palette: AlertPaletteVisual,
}

// 保存 Alert 每帧只解析一次的主题颜色和圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedAlertVisual {
    pub(crate) background: Color,
    pub(crate) status: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) primary: Color,
    pub(crate) container_radius: f32,
    pub(crate) interaction_radius: f32,
}

impl AlertVisual {
    pub(crate) fn resolve(
        self,
        level: StatusLevel,
        tokens: &dyn ThemeTokens,
    ) -> ResolvedAlertVisual {
        let status = self.palette.statuses[match level {
            StatusLevel::Success => 0,
            StatusLevel::Info => 1,
            StatusLevel::Warning => 2,
            StatusLevel::Error => 3,
        }];
        ResolvedAlertVisual {
            background: status.background.resolve(tokens),
            status: status.foreground.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            container_radius: self.chrome.container_radius.resolve(tokens),
            interaction_radius: self.chrome.interaction_radius.resolve(tokens),
        }
    }
}

pub(crate) const fn alert_defaults_visual(
    width: f32,
    base_height: f32,
    description_height: f32,
    show_icon: bool,
    banner: bool,
) -> AlertDefaultsVisual {
    AlertDefaultsVisual {
        width,
        base_height,
        description_height,
        show_icon,
        banner,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn alert_layout_visual(
    close_width: f32,
    close_action_gap: f32,
    content_right_gap: f32,
    action_width: f32,
    action_content_gap: f32,
    icon_max_width: f32,
    icon_gap: f32,
    no_icon_left: f32,
    content_vertical_inset: f32,
    content_vertical_ratio: f32,
    message_height_ratio: f32,
    accent_x: f32,
    accent_y: f32,
    accent_width: f32,
    icon_x: f32,
) -> AlertLayoutVisual {
    AlertLayoutVisual {
        close_width,
        close_action_gap,
        content_right_gap,
        action_width,
        action_content_gap,
        icon_max_width,
        icon_gap,
        no_icon_left,
        content_vertical_inset,
        content_vertical_ratio,
        message_height_ratio,
        accent_x,
        accent_y,
        accent_width,
        icon_x,
    }
}

pub(crate) const fn alert_chrome_visual(
    accent_radius: f32,
    action_inset: f32,
    close_inset: f32,
    focus_stroke: f32,
    container_radius: AlertRadiusRole,
    interaction_radius: AlertRadiusRole,
) -> AlertChromeVisual {
    AlertChromeVisual {
        accent_radius,
        action_inset,
        close_inset,
        focus_stroke,
        container_radius,
        interaction_radius,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn alert_typography_visual(
    message: f32,
    description: f32,
    action: f32,
    status_icon: f32,
    close_icon: f32,
    success_icon: &'static str,
    info_icon: &'static str,
    warning_icon: &'static str,
    error_icon: &'static str,
    close_icon_name: &'static str,
) -> AlertTypographyVisual {
    AlertTypographyVisual {
        message,
        description,
        action,
        status_icon,
        close_icon,
        success_icon,
        info_icon,
        warning_icon,
        error_icon,
        close_icon_name,
    }
}

pub(crate) const fn alert_status_palette_visual(
    background: ColorValue,
    foreground: ColorValue,
) -> AlertStatusPaletteVisual {
    AlertStatusPaletteVisual {
        background,
        foreground,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn alert_palette_visual(
    success: AlertStatusPaletteVisual,
    info: AlertStatusPaletteVisual,
    warning: AlertStatusPaletteVisual,
    error: AlertStatusPaletteVisual,
    text: ColorValue,
    text_secondary: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    primary: ColorValue,
) -> AlertPaletteVisual {
    AlertPaletteVisual {
        statuses: [success, info, warning, error],
        text,
        text_secondary,
        fill_secondary,
        fill_tertiary,
        primary,
    }
}

pub(crate) const fn alert_visual(
    defaults: AlertDefaultsVisual,
    layout: AlertLayoutVisual,
    chrome: AlertChromeVisual,
    typography: AlertTypographyVisual,
    palette: AlertPaletteVisual,
) -> AlertVisual {
    AlertVisual {
        defaults,
        layout,
        chrome,
        typography,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接书写的圆角、图标和主题角色。
pub(crate) const fn alert_radius_normal() -> AlertRadiusRole {
    AlertRadiusRole::Normal
}
pub(crate) const fn alert_radius_small() -> AlertRadiusRole {
    AlertRadiusRole::Small
}
pub(crate) const fn alert_success_icon() -> &'static str {
    "check-circle"
}
pub(crate) const fn alert_info_icon() -> &'static str {
    "info"
}
pub(crate) const fn alert_warning_icon() -> &'static str {
    "alert-triangle"
}
pub(crate) const fn alert_error_icon() -> &'static str {
    "x-circle"
}
pub(crate) const fn alert_close_icon() -> &'static str {
    "x"
}
pub(crate) const fn alert_success_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::SuccessBg)
}
pub(crate) const fn alert_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
pub(crate) const fn alert_info_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::InfoBg)
}
pub(crate) const fn alert_info() -> ColorValue {
    ColorValue::Palette(PaletteColor::Info)
}
pub(crate) const fn alert_warning_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::WarningBg)
}
pub(crate) const fn alert_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn alert_error_bg() -> ColorValue {
    ColorValue::Palette(PaletteColor::ErrorBg)
}
pub(crate) const fn alert_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(crate) const fn alert_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn alert_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn alert_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn alert_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn alert_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

pub(crate) static DEFAULT_ALERT_VISUAL: AlertVisual = alert_visual(
    alert_defaults_visual(300.0, 36.0, 18.0, true, false),
    alert_layout_visual(
        36.0, 4.0, 8.0, 64.0, 8.0, 28.0, 8.0, 14.0, 4.0, 0.5, 0.52, 2.0, 4.0, 3.0, 8.0,
    ),
    alert_chrome_visual(
        1.5,
        3.0,
        4.0,
        2.0,
        AlertRadiusRole::Normal,
        AlertRadiusRole::Small,
    ),
    alert_typography_visual(
        14.0,
        12.0,
        13.0,
        14.0,
        14.0,
        "check-circle",
        "info",
        "alert-triangle",
        "x-circle",
        "x",
    ),
    alert_palette_visual(
        alert_status_palette_visual(
            ColorValue::Palette(PaletteColor::SuccessBg),
            ColorValue::Palette(PaletteColor::Success),
        ),
        alert_status_palette_visual(
            ColorValue::Palette(PaletteColor::InfoBg),
            ColorValue::Palette(PaletteColor::Info),
        ),
        alert_status_palette_visual(
            ColorValue::Palette(PaletteColor::WarningBg),
            ColorValue::Palette(PaletteColor::Warning),
        ),
        alert_status_palette_visual(
            ColorValue::Palette(PaletteColor::ErrorBg),
            ColorValue::Palette(PaletteColor::Error),
        ),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::FillSecondary),
        ColorValue::Neutral(NeutralRole::FillTertiary),
        ColorValue::Palette(PaletteColor::Primary),
    ),
);

// 首次 UIX 构建固化声明值，全部 Alert 实例共享一份视觉表。
pub(crate) static UIX_ALERT_VISUAL: OnceLock<AlertVisual> = OnceLock::new();
