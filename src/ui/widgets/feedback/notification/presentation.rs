//! Notification 的 UIX 静态视觉契约与每帧主题解析。

use crate::draw::Color;
use crate::platform::capabilities::StatusLevel;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};
use crate::ui::{Placement, ThemeTokens};

// 保存通知的默认停靠位置与展示时长。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotificationDefaultsVisual {
    pub(crate) placement: Placement,
    pub(crate) duration_ms: u64,
}

// 保存通知队列、单项、内容与操作控件的全部静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotificationLayoutVisual {
    pub(crate) max_width: f32,
    pub(crate) horizontal_inset: f32,
    pub(crate) vertical_inset: f32,
    pub(crate) item_gap: f32,
    pub(crate) shadow_margin: f32,
    pub(crate) compact_height: f32,
    pub(crate) detailed_height: f32,
    pub(crate) accent_vertical_inset: f32,
    pub(crate) accent_width: f32,
    pub(crate) accent_radius: f32,
    pub(crate) icon_inset: f32,
    pub(crate) icon_max_width: f32,
    pub(crate) icon_content_gap: f32,
    pub(crate) title_top_inset: f32,
    pub(crate) title_height: f32,
    pub(crate) action_inset: f32,
    pub(crate) close_inset: f32,
    pub(crate) close_default_width: f32,
    pub(crate) action_horizontal_padding: f32,
    pub(crate) close_horizontal_padding: f32,
    pub(crate) action_min_width: f32,
    pub(crate) action_max_width: f32,
    pub(crate) close_min_width: f32,
    pub(crate) close_max_width: f32,
    pub(crate) control_gap: f32,
    pub(crate) content_trailing_gap: f32,
}

// 保存标题、描述、操作与图标的字号。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotificationTypographyVisual {
    title: NotificationFontRole,
    pub(crate) description: f32,
    pub(crate) action: f32,
    pub(crate) close_label: f32,
    pub(crate) status_icon: f32,
    pub(crate) close_icon: f32,
}

// 保存默认进场与离场时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotificationMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) leave_duration: f64,
}

// 保存四类状态图标和关闭图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationIconsVisual {
    pub(crate) success: &'static str,
    pub(crate) info: &'static str,
    pub(crate) warning: &'static str,
    pub(crate) error: &'static str,
    pub(crate) close: &'static str,
}

// 保存主题字号或固定字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum NotificationFontRole {
    Body,
}

impl NotificationFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Body => tokens.font_size(),
        }
    }
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationRadiusRole {
    Large,
    Small,
}

impl NotificationRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Large => tokens.border_radius_lg(),
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存主题阴影角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NotificationShadowRole {
    Primary,
}

impl NotificationShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Primary => tokens.box_shadow(),
        }
    }
}

// 保存浮层层级、描边、圆角与阴影角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotificationChromeVisual {
    pub(crate) overlay_z: i32,
    pub(crate) panel_stroke: f32,
    radius: NotificationRadiusRole,
    control_radius: NotificationRadiusRole,
    shadow: NotificationShadowRole,
}

// 保存 Notification 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NotificationPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    success: ColorValue,
    info: ColorValue,
    warning: ColorValue,
    error: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    text_quaternary: ColorValue,
}

// 全部 Notification 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NotificationVisual {
    pub(crate) defaults: NotificationDefaultsVisual,
    pub(crate) layout: NotificationLayoutVisual,
    pub(crate) typography: NotificationTypographyVisual,
    pub(crate) motion: NotificationMotionVisual,
    pub(crate) icons: NotificationIconsVisual,
    pub(crate) chrome: NotificationChromeVisual,
    palette: NotificationPaletteVisual,
}

crate::uix_items!("src/ui/widgets/feedback/notification/notification.uix");

// 保存一次绘制开始时解析的主题值，供全部可见通知共享。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedNotificationVisual {
    pub(crate) background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) success: Color,
    pub(crate) info: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) radius: f32,
    pub(crate) control_radius: f32,
    pub(crate) title_font_size: f32,
    pub(crate) shadow: ShadowToken,
}

impl NotificationVisual {
    // 每帧只解析一次主题令牌，避免每条通知重复虚调用或取锁。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedNotificationVisual {
        ResolvedNotificationVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            info: self.palette.info.resolve(tokens),
            warning: self.palette.warning.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            control_radius: self.chrome.control_radius.resolve(tokens),
            title_font_size: self.typography.title.resolve(tokens),
            shadow: self.chrome.shadow.resolve(tokens),
        }
    }

    // 返回状态对应的 UIX 图标与已解析主题色。
    pub(crate) fn status_visual(
        &self,
        resolved: &ResolvedNotificationVisual,
        status: StatusLevel,
    ) -> (&'static str, Color) {
        match status {
            StatusLevel::Success => (self.icons.success, resolved.success),
            StatusLevel::Info => (self.icons.info, resolved.info),
            StatusLevel::Warning => (self.icons.warning, resolved.warning),
            StatusLevel::Error => (self.icons.error, resolved.error),
        }
    }
}

pub(crate) const fn notification_placement_top_right() -> Placement {
    Placement::TopRight
}
pub(crate) const fn notification_font_body() -> NotificationFontRole {
    NotificationFontRole::Body
}
pub(crate) const fn notification_radius_large() -> NotificationRadiusRole {
    NotificationRadiusRole::Large
}
pub(crate) const fn notification_radius_small() -> NotificationRadiusRole {
    NotificationRadiusRole::Small
}
pub(crate) const fn notification_shadow_primary() -> NotificationShadowRole {
    NotificationShadowRole::Primary
}
pub(crate) const fn notification_bg_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn notification_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
pub(crate) const fn notification_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn notification_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn notification_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
pub(crate) const fn notification_info() -> ColorValue {
    ColorValue::Palette(PaletteColor::Info)
}
pub(crate) const fn notification_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn notification_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(crate) const fn notification_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn notification_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn notification_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
