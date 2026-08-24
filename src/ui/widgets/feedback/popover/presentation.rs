//! Popover 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::style::{ColorValue, PaletteColor};
use crate::ui::theme::{NeutralRole, ShadowToken};

use super::PopoverPlacement;

// 保存 UIX 声明的气泡与触发器固有尺寸及默认视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopoverDefaultsVisual {
    pub(crate) popup_width: f32,
    pub(crate) popup_height: f32,
    pub(crate) trigger_width: f32,
    pub(crate) trigger_height: f32,
    pub(crate) placement: PopoverPlacement,
    pub(crate) arrow: bool,
}

// 保存定位、内容分区、阴影脏区与箭头使用的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopoverLayoutVisual {
    pub(crate) arrow_gap: f32,
    pub(crate) plain_gap: f32,
    pub(crate) center_ratio: f32,
    pub(crate) fallback_offset_popups: f32,
    pub(crate) fallback_span_popups: f32,
    pub(crate) shadow_expand: f32,
    pub(crate) content_inset: f32,
    pub(crate) title_height: f32,
    pub(crate) divider_thickness: f32,
    pub(crate) arrow_size: f32,
}

// 保存触发器描边、浮层层级、默认标签与主题装饰角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopoverChromeVisual {
    pub(crate) trigger_stroke: f32,
    pub(crate) focus_stroke: f32,
    pub(crate) overlay_z: i32,
    pub(crate) default_trigger_label: &'static str,
    radius: PopoverRadiusRole,
    shadow: PopoverShadowRole,
}

// 保存触发器、标题和正文的静态排版角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopoverTypographyVisual {
    trigger: PopoverFontRole,
    title: PopoverFontRole,
    content: PopoverFontRole,
}

// 保存默认进入与退出的静态时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopoverMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) exit_duration: f64,
}

// 保存 Popover 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PopoverPaletteVisual {
    popup_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    primary: ColorValue,
}

// 保存字体令牌或 UIX 固定字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum PopoverFontRole {
    Normal,
    Small,
    Fixed(f32),
}

impl PopoverFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Normal => tokens.font_size(),
            Self::Small => tokens.font_size_sm(),
            Self::Fixed(value) => value,
        }
    }
}

// 保存圆角令牌的静态语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopoverRadiusRole {
    Small,
}

impl PopoverRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存阴影令牌的静态语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PopoverShadowRole {
    Secondary,
}

impl PopoverShadowRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> ShadowToken {
        match self {
            Self::Secondary => tokens.box_shadow_secondary(),
        }
    }
}

// 全部 Popover 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PopoverVisual {
    pub(crate) defaults: PopoverDefaultsVisual,
    pub(crate) layout: PopoverLayoutVisual,
    pub(crate) chrome: PopoverChromeVisual,
    pub(crate) motion: PopoverMotionVisual,
    typography: PopoverTypographyVisual,
    palette: PopoverPaletteVisual,
}

// 保存 Popover 每帧一次解析得到的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedPopoverVisual {
    pub(crate) popup_background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) primary: Color,
    pub(crate) radius: f32,
    pub(crate) shadow: ShadowToken,
    pub(crate) trigger_font_size: f32,
    pub(crate) title_font_size: f32,
    pub(crate) content_font_size: f32,
}

impl PopoverVisual {
    pub(crate) fn resolve(
        &self,
        tokens: &dyn ThemeTokens,
        popup_present: bool,
    ) -> ResolvedPopoverVisual {
        // 隐藏气泡不解析只供气泡内容消费的主题值。
        let (popup_background, text, shadow, title_font_size, content_font_size) = if popup_present
        {
            (
                self.palette.popup_background.resolve(tokens),
                self.palette.text.resolve(tokens),
                self.chrome.shadow.resolve(tokens),
                self.typography.title.resolve(tokens),
                self.typography.content.resolve(tokens),
            )
        } else {
            (
                Color::transparent(),
                Color::transparent(),
                ShadowToken::none(),
                0.0,
                0.0,
            )
        };
        ResolvedPopoverVisual {
            popup_background,
            border: self.palette.border.resolve(tokens),
            text,
            text_secondary: self.palette.text_secondary.resolve(tokens),
            fill_secondary: self.palette.fill_secondary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            shadow,
            trigger_font_size: self.typography.trigger.resolve(tokens),
            title_font_size,
            content_font_size,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn popover_defaults_visual(
    popup_width: f32,
    popup_height: f32,
    trigger_width: f32,
    trigger_height: f32,
    placement: PopoverPlacement,
    arrow: bool,
) -> PopoverDefaultsVisual {
    PopoverDefaultsVisual {
        popup_width,
        popup_height,
        trigger_width,
        trigger_height,
        placement,
        arrow,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn popover_layout_visual(
    arrow_gap: f32,
    plain_gap: f32,
    center_ratio: f32,
    fallback_offset_popups: f32,
    fallback_span_popups: f32,
    shadow_expand: f32,
    content_inset: f32,
    title_height: f32,
    divider_thickness: f32,
    arrow_size: f32,
) -> PopoverLayoutVisual {
    PopoverLayoutVisual {
        arrow_gap,
        plain_gap,
        center_ratio,
        fallback_offset_popups,
        fallback_span_popups,
        shadow_expand,
        content_inset,
        title_height,
        divider_thickness,
        arrow_size,
    }
}

pub(crate) const fn popover_chrome_visual(
    trigger_stroke: f32,
    focus_stroke: f32,
    overlay_z: f32,
    default_trigger_label: &'static str,
    radius: PopoverRadiusRole,
    shadow: PopoverShadowRole,
) -> PopoverChromeVisual {
    PopoverChromeVisual {
        trigger_stroke,
        focus_stroke,
        overlay_z: overlay_z as i32,
        default_trigger_label,
        radius,
        shadow,
    }
}

pub(crate) const fn popover_typography_visual(
    trigger: PopoverFontRole,
    title: PopoverFontRole,
    content: PopoverFontRole,
) -> PopoverTypographyVisual {
    PopoverTypographyVisual {
        trigger,
        title,
        content,
    }
}

pub(crate) const fn popover_motion_visual(
    enter_duration: f64,
    exit_duration: f64,
) -> PopoverMotionVisual {
    PopoverMotionVisual {
        enter_duration,
        exit_duration,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn popover_palette_visual(
    popup_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    primary: ColorValue,
) -> PopoverPaletteVisual {
    PopoverPaletteVisual {
        popup_background,
        border,
        text,
        text_secondary,
        fill_secondary,
        fill_tertiary,
        primary,
    }
}

pub(crate) const fn popover_visual(
    defaults: PopoverDefaultsVisual,
    layout: PopoverLayoutVisual,
    chrome: PopoverChromeVisual,
    typography: PopoverTypographyVisual,
    motion: PopoverMotionVisual,
    palette: PopoverPaletteVisual,
) -> PopoverVisual {
    PopoverVisual {
        defaults,
        layout,
        chrome,
        motion,
        typography,
        palette,
    }
}

pub(crate) const fn popover_placement_top() -> PopoverPlacement {
    PopoverPlacement::Top
}
pub(crate) const fn popover_radius_small() -> PopoverRadiusRole {
    PopoverRadiusRole::Small
}
pub(crate) const fn popover_shadow_secondary() -> PopoverShadowRole {
    PopoverShadowRole::Secondary
}
pub(crate) const fn popover_font_normal() -> PopoverFontRole {
    PopoverFontRole::Normal
}
pub(crate) const fn popover_font_small() -> PopoverFontRole {
    PopoverFontRole::Small
}
pub(crate) const fn popover_font_fixed(value: f32) -> PopoverFontRole {
    PopoverFontRole::Fixed(value)
}
pub(crate) const fn popover_default_trigger_label() -> &'static str {
    "Popover"
}
pub(crate) const fn popover_bg_elevated() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn popover_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn popover_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn popover_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn popover_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn popover_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn popover_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}

pub(crate) static DEFAULT_POPOVER_VISUAL: PopoverVisual = popover_visual(
    popover_defaults_visual(220.0, 100.0, 80.0, 28.0, PopoverPlacement::Top, true),
    popover_layout_visual(10.0, 4.0, 0.5, 2.0, 5.0, 12.0, 12.0, 32.0, 1.0, 8.0),
    popover_chrome_visual(
        1.0,
        2.0,
        900.0,
        "Popover",
        PopoverRadiusRole::Small,
        PopoverShadowRole::Secondary,
    ),
    popover_typography_visual(
        PopoverFontRole::Small,
        PopoverFontRole::Normal,
        PopoverFontRole::Fixed(12.0),
    ),
    popover_motion_visual(0.15, 0.1),
    popover_palette_visual(
        ColorValue::Neutral(NeutralRole::BgElevated),
        ColorValue::Neutral(NeutralRole::Border),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::FillSecondary),
        ColorValue::Neutral(NeutralRole::FillTertiary),
        ColorValue::Palette(PaletteColor::Primary),
    ),
);

// 首次 UIX 构建固化声明值，全部 Popover 实例共享一份视觉表。
pub(crate) static UIX_POPOVER_VISUAL: OnceLock<PopoverVisual> = OnceLock::new();
