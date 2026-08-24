//! Drawer 的 UIX 静态视觉契约与按状态主题解析。

use crate::draw::Color;
use crate::platform::windowing::ControlSize;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

use super::DrawerPlacement;

// 保存三档面板尺寸和默认行为视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawerDefaultsVisual {
    pub(crate) small_width: f32,
    pub(crate) small_height: f32,
    pub(crate) medium_width: f32,
    pub(crate) medium_height: f32,
    pub(crate) large_width: f32,
    pub(crate) large_height: f32,
    pub(crate) placement: DrawerPlacement,
    pub(crate) closable: bool,
    pub(crate) mask_closable: bool,
    pub(crate) mask: bool,
    pub(crate) footer_visible: bool,
}

impl DrawerDefaultsVisual {
    // 从 UIX 唯一尺寸表解析当前控件档位。
    pub(crate) const fn dimensions(self, size: ControlSize) -> (f32, f32) {
        match size {
            ControlSize::Small => (self.small_width, self.small_height),
            ControlSize::Medium => (self.medium_width, self.medium_height),
            ControlSize::Large => (self.large_width, self.large_height),
        }
    }
}

// 保存触发器、面板分区、内容槽与兼容回退所需的全部静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawerLayoutVisual {
    pub(crate) trigger_width: f32,
    pub(crate) trigger_height: f32,
    pub(crate) open_vertical_extent: f32,
    pub(crate) open_horizontal_extent: f32,
    pub(crate) surface_fallback_width: f32,
    pub(crate) surface_fallback_height: f32,
    pub(crate) overlay_fallback_origin: f32,
    pub(crate) overlay_fallback_extent: f32,
    pub(crate) header_height: f32,
    pub(crate) close_width: f32,
    pub(crate) extra_max_width: f32,
    pub(crate) title_inset: f32,
    pub(crate) close_inset: f32,
    pub(crate) divider_thickness: f32,
    pub(crate) footer_height: f32,
    pub(crate) footer_side_inset: f32,
    pub(crate) footer_button_max_width: f32,
    pub(crate) footer_vertical_padding: f32,
    pub(crate) footer_button_max_height: f32,
    pub(crate) body_padding: f32,
}

// 保存触发文案、标题、附加文字、图标和底部动作的字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawerTypographyVisual {
    trigger: DrawerFontRole,
    title: DrawerFontRole,
    extra: DrawerFontRole,
    pub(crate) close_icon: f32,
    footer: DrawerFontRole,
}

// 保存默认进场和离场的时序与滑动距离。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawerMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) leave_duration: f64,
    pub(crate) distance: f32,
}

// 保存 Drawer 使用的静态图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawerIconsVisual {
    pub(crate) close: &'static str,
}

// 保存浮层层级、描边、圆角和关闭态展示文案。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawerChromeVisual {
    pub(crate) overlay_z: i32,
    pub(crate) panel_stroke: f32,
    pub(crate) control_radius: f32,
    pub(crate) trigger_label: &'static str,
    panel_radius: DrawerRadiusRole,
    trigger_radius: DrawerRadiusRole,
}

// 保存 Drawer 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawerPaletteVisual {
    mask: ColorValue,
    panel_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    primary_hover: ColorValue,
    primary_active: ColorValue,
    fill_secondary: ColorValue,
    fill_tertiary: ColorValue,
    white: ColorValue,
}

// 保存 UIX 字号令牌或固定字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum DrawerFontRole {
    Normal,
    Large,
    Fixed(f32),
}

impl DrawerFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Normal => tokens.font_size(),
            Self::Large => tokens.font_size_lg(),
            Self::Fixed(value) => value,
        }
    }
}

// 保存 UIX 圆角主题角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DrawerRadiusRole {
    Normal,
    Large,
}

impl DrawerRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Normal => tokens.border_radius(),
            Self::Large => tokens.border_radius_lg(),
        }
    }
}

// 全部 Drawer 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawerVisual {
    pub(crate) defaults: DrawerDefaultsVisual,
    pub(crate) layout: DrawerLayoutVisual,
    pub(crate) typography: DrawerTypographyVisual,
    pub(crate) motion: DrawerMotionVisual,
    pub(crate) icons: DrawerIconsVisual,
    pub(crate) chrome: DrawerChromeVisual,
    palette: DrawerPaletteVisual,
}

crate::uix_items!("src/ui/widgets/feedback/drawer/drawer.uix");

// 保存关闭态触发器一次主题解析的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedDrawerTriggerVisual {
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) primary_active: Color,
    pub(crate) white: Color,
    pub(crate) radius: f32,
    pub(crate) font_size: f32,
}

// 保存打开态面板一次主题解析的结果；未使用分支保持透明占位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedDrawerPanelVisual {
    pub(crate) mask: Color,
    pub(crate) background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) primary: Color,
    pub(crate) white: Color,
    pub(crate) radius: f32,
    pub(crate) title_font_size: f32,
    pub(crate) extra_font_size: f32,
    pub(crate) footer_font_size: f32,
}

impl DrawerVisual {
    // 关闭态只解析触发器实际需要的主题角色。
    pub(crate) fn resolve_trigger(&self, tokens: &dyn ThemeTokens) -> ResolvedDrawerTriggerVisual {
        ResolvedDrawerTriggerVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            primary_active: self.palette.primary_active.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            radius: self.chrome.trigger_radius.resolve(tokens),
            font_size: self.typography.trigger.resolve(tokens),
        }
    }

    // 打开态按可见分支解析主题，避免隐藏关闭区、附加文字或 footer 的虚调用。
    pub(crate) fn resolve_panel(
        &self,
        tokens: &dyn ThemeTokens,
        has_mask: bool,
        has_close_control: bool,
        has_extra_text: bool,
        has_footer: bool,
    ) -> ResolvedDrawerPanelVisual {
        // 关闭图标与附加文案共享次要文字色，但字号只在附加文案存在时解析。
        let has_secondary_text = has_close_control || has_extra_text;
        ResolvedDrawerPanelVisual {
            mask: if has_mask {
                self.palette.mask.resolve(tokens)
            } else {
                Color::transparent()
            },
            background: self.palette.panel_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: if has_secondary_text {
                self.palette.text_secondary.resolve(tokens)
            } else {
                Color::transparent()
            },
            fill_secondary: if has_close_control {
                self.palette.fill_secondary.resolve(tokens)
            } else {
                Color::transparent()
            },
            fill_tertiary: if has_close_control {
                self.palette.fill_tertiary.resolve(tokens)
            } else {
                Color::transparent()
            },
            primary: if has_footer {
                self.palette.primary.resolve(tokens)
            } else {
                Color::transparent()
            },
            white: if has_footer {
                self.palette.white.resolve(tokens)
            } else {
                Color::transparent()
            },
            radius: self.chrome.panel_radius.resolve(tokens),
            title_font_size: self.typography.title.resolve(tokens),
            extra_font_size: if has_extra_text {
                self.typography.extra.resolve(tokens)
            } else {
                0.0
            },
            footer_font_size: if has_footer {
                self.typography.footer.resolve(tokens)
            } else {
                0.0
            },
        }
    }
}

pub(crate) const fn drawer_placement_right() -> DrawerPlacement {
    DrawerPlacement::Right
}
pub(crate) const fn drawer_font_normal() -> DrawerFontRole {
    DrawerFontRole::Normal
}
pub(crate) const fn drawer_font_large() -> DrawerFontRole {
    DrawerFontRole::Large
}
pub(crate) const fn drawer_font_fixed(value: f32) -> DrawerFontRole {
    DrawerFontRole::Fixed(value)
}
pub(crate) const fn drawer_radius_normal() -> DrawerRadiusRole {
    DrawerRadiusRole::Normal
}
pub(crate) const fn drawer_radius_large() -> DrawerRadiusRole {
    DrawerRadiusRole::Large
}
pub(crate) const fn drawer_bg_mask() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgMask)
}
pub(crate) const fn drawer_bg_container() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn drawer_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
pub(crate) const fn drawer_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn drawer_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn drawer_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn drawer_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
pub(crate) const fn drawer_primary_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryActive)
}
pub(crate) const fn drawer_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn drawer_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn drawer_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
