//! Message 的 UIX 静态视觉契约与每帧主题解析。

use crate::draw::Color;
use crate::platform::capabilities::StatusLevel;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};
use crate::ui::{Placement, ThemeTokens};

// 保存 Message 的默认放置位置与四类提示时长。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MessageDefaultsVisual {
    pub(crate) placement: Placement,
    pub(crate) success_duration_ms: u64,
    pub(crate) info_duration_ms: u64,
    pub(crate) warning_duration_ms: u64,
    pub(crate) error_duration_ms: u64,
}

// 保存提示队列、单项、控件和内容区域的全部静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MessageLayoutVisual {
    pub(crate) max_width: f32,
    pub(crate) item_height: f32,
    pub(crate) item_stride: f32,
    pub(crate) surface_inset: f32,
    pub(crate) shadow_margin: f32,
    pub(crate) accent_vertical_inset: f32,
    pub(crate) accent_width: f32,
    pub(crate) accent_radius: f32,
    pub(crate) icon_inset: f32,
    pub(crate) icon_max_width: f32,
    pub(crate) icon_content_gap: f32,
    pub(crate) action_inset: f32,
    pub(crate) close_inset: f32,
    pub(crate) close_width: f32,
    pub(crate) action_horizontal_padding: f32,
    pub(crate) action_min_width: f32,
    pub(crate) action_max_width: f32,
    pub(crate) control_gap: f32,
    pub(crate) content_trailing_gap: f32,
}

// 保存正文、动作与图标使用的静态字号。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MessageTypographyVisual {
    pub(crate) body: f32,
    pub(crate) action: f32,
    pub(crate) status_icon: f32,
    pub(crate) close_icon: f32,
}

// 保存默认进场与离场时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MessageMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) leave_duration: f64,
}

// 保存四类状态图标和关闭图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MessageIconsVisual {
    pub(crate) success: &'static str,
    pub(crate) info: &'static str,
    pub(crate) warning: &'static str,
    pub(crate) error: &'static str,
    pub(crate) close: &'static str,
}

// 保存 Message 使用的主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MessagePaletteVisual {
    background: ColorValue,
    text: ColorValue,
    success: ColorValue,
    info: ColorValue,
    warning: ColorValue,
    error: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    text_quaternary: ColorValue,
}

// 保存主题圆角角色，避免 UIX 固化当前主题的数值。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageRadiusRole {
    Large,
    Small,
}

impl MessageRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Large => tokens.border_radius_lg(),
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存主题阴影角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageShadowRole {
    Primary,
}

impl MessageShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Primary => tokens.box_shadow(),
        }
    }
}

// 保存浮层层级、圆角与阴影角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MessageChromeVisual {
    pub(crate) overlay_z: i32,
    radius: MessageRadiusRole,
    control_radius: MessageRadiusRole,
    shadow: MessageShadowRole,
}

// 全部 Message 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct MessageVisual {
    pub(crate) defaults: MessageDefaultsVisual,
    pub(crate) layout: MessageLayoutVisual,
    pub(crate) typography: MessageTypographyVisual,
    pub(crate) motion: MessageMotionVisual,
    pub(crate) icons: MessageIconsVisual,
    chrome: MessageChromeVisual,
    palette: MessagePaletteVisual,
}

crate::uix_items!("src/ui/widgets/feedback/message/message.uix");

// 保存一次绘制开始时解析的主题值，供全部可见提示共享。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedMessageVisual {
    pub(crate) background: Color,
    pub(crate) text: Color,
    pub(crate) success: Color,
    pub(crate) info: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) radius: f32,
    pub(crate) control_radius: f32,
    pub(crate) shadow: ShadowToken,
}

impl MessageVisual {
    // 只公开浮层登记所需的窄层级值，不泄漏完整 chrome 结构。
    pub(crate) const fn overlay_z(&self) -> i32 {
        self.chrome.overlay_z
    }

    // 每帧只解析一次主题令牌，避免每条提示重复虚调用或重复取锁。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedMessageVisual {
        ResolvedMessageVisual {
            background: self.palette.background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            info: self.palette.info.resolve(tokens),
            warning: self.palette.warning.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            control_radius: self.chrome.control_radius.resolve(tokens),
            shadow: self.chrome.shadow.resolve(tokens),
        }
    }

    // 返回状态对应的 UIX 图标与已解析主题色。
    pub(crate) fn status_visual(
        &self,
        resolved: &ResolvedMessageVisual,
        status: StatusLevel,
    ) -> (&'static str, Color) {
        match status {
            StatusLevel::Success => (self.icons.success, resolved.success),
            StatusLevel::Info => (self.icons.info, resolved.info),
            StatusLevel::Warning => (self.icons.warning, resolved.warning),
            StatusLevel::Error => (self.icons.error, resolved.error),
        }
    }

    // 返回指定状态的 UIX 默认展示时长。
    pub(crate) const fn duration_ms(&self, status: StatusLevel) -> u64 {
        match status {
            StatusLevel::Success => self.defaults.success_duration_ms,
            StatusLevel::Info => self.defaults.info_duration_ms,
            StatusLevel::Warning => self.defaults.warning_duration_ms,
            StatusLevel::Error => self.defaults.error_duration_ms,
        }
    }
}

pub(crate) const fn message_placement_top() -> Placement {
    Placement::Top
}
pub(crate) const fn message_radius_large() -> MessageRadiusRole {
    MessageRadiusRole::Large
}
pub(crate) const fn message_radius_small() -> MessageRadiusRole {
    MessageRadiusRole::Small
}
pub(crate) const fn message_shadow_primary() -> MessageShadowRole {
    MessageShadowRole::Primary
}
pub(crate) const fn message_bg_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn message_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn message_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
pub(crate) const fn message_info() -> ColorValue {
    ColorValue::Palette(PaletteColor::Info)
}
pub(crate) const fn message_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn message_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(crate) const fn message_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn message_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn message_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
