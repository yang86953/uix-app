//! Modal 的 UIX 静态视觉契约与按状态主题解析。

use crate::draw::Color;
use crate::platform::windowing::ControlSize;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存三档对话框尺寸和默认行为视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ModalDefaultsVisual {
    pub(crate) small_width: f32,
    pub(crate) small_height: f32,
    pub(crate) medium_width: f32,
    pub(crate) medium_height: f32,
    pub(crate) large_width: f32,
    pub(crate) large_height: f32,
    pub(crate) closable: bool,
    pub(crate) mask_closable: bool,
    pub(crate) footer_visible: bool,
    pub(crate) centered: bool,
    pub(crate) overlay: bool,
    pub(crate) destroy_on_close: bool,
}

impl ModalDefaultsVisual {
    // 从 UIX 唯一尺寸表解析当前控件档位。
    pub(crate) const fn dimensions(self, size: ControlSize) -> (f32, f32) {
        match size {
            ControlSize::Small => (self.small_width, self.small_height),
            ControlSize::Medium => (self.medium_width, self.medium_height),
            ControlSize::Large => (self.large_width, self.large_height),
        }
    }
}

// 保存触发器、对话框分区、内容槽和窄表面收敛所需的全部静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ModalLayoutVisual {
    pub(crate) trigger_width: f32,
    pub(crate) trigger_height: f32,
    pub(crate) surface_fallback_width: f32,
    pub(crate) surface_fallback_height: f32,
    pub(crate) overlay_fallback_origin: f32,
    pub(crate) overlay_fallback_extent: f32,
    pub(crate) safe_margin: f32,
    pub(crate) safe_margin_ratio: f32,
    pub(crate) header_height: f32,
    pub(crate) footer_height: f32,
    pub(crate) close_width: f32,
    pub(crate) title_inset: f32,
    pub(crate) close_inset: f32,
    pub(crate) divider_thickness: f32,
    pub(crate) body_padding: f32,
    pub(crate) footer_side_inset: f32,
    pub(crate) footer_side_inset_ratio: f32,
    pub(crate) footer_gap: f32,
    pub(crate) footer_gap_ratio: f32,
    pub(crate) footer_button_max_width: f32,
    pub(crate) footer_button_vertical_inset: f32,
    pub(crate) footer_button_max_height: f32,
}

// 保存触发文案、标题、关闭图标和底部动作的字号角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ModalTypographyVisual {
    trigger: ModalFontRole,
    title: ModalFontRole,
    pub(crate) close_icon: f32,
    footer: ModalFontRole,
}

// 保存默认进场和离场时长；缩放类型仍由 Rust 动画内核执行。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ModalMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) leave_duration: f64,
}

// 保存 Modal 使用的静态图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModalIconsVisual {
    pub(crate) close: &'static str,
}

// 保存浮层层级、描边、圆角和内置展示文案。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ModalChromeVisual {
    pub(crate) overlay_z: i32,
    pub(crate) panel_stroke: f32,
    pub(crate) trigger_label: &'static str,
    panel_radius: ModalRadiusRole,
    trigger_radius: ModalRadiusRole,
    control_radius: ModalRadiusRole,
}

// 保存 Modal 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ModalPaletteVisual {
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
pub(crate) enum ModalFontRole {
    Large,
    Fixed(f32),
}

impl ModalFontRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Large => tokens.font_size_lg(),
            Self::Fixed(value) => value,
        }
    }
}

// 保存 UIX 圆角主题角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ModalRadiusRole {
    Normal,
    Small,
    Large,
}

impl ModalRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Normal => tokens.border_radius(),
            Self::Small => tokens.border_radius_sm(),
            Self::Large => tokens.border_radius_lg(),
        }
    }
}

// 全部 Modal 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ModalVisual {
    pub(crate) defaults: ModalDefaultsVisual,
    pub(crate) layout: ModalLayoutVisual,
    pub(crate) typography: ModalTypographyVisual,
    pub(crate) motion: ModalMotionVisual,
    pub(crate) icons: ModalIconsVisual,
    pub(crate) chrome: ModalChromeVisual,
    palette: ModalPaletteVisual,
}

crate::uix_items!("src/ui/widgets/feedback/modal/modal.uix");

// 保存关闭态触发器一次主题解析的结果。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedModalTriggerVisual {
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) primary_active: Color,
    pub(crate) white: Color,
    pub(crate) radius: f32,
    pub(crate) font_size: f32,
}

// 保存打开态对话框一次主题解析的结果；隐藏分支保持透明占位。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedModalDialogVisual {
    pub(crate) mask: Color,
    pub(crate) background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) fill_secondary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) primary: Color,
    pub(crate) primary_hover: Color,
    pub(crate) primary_active: Color,
    pub(crate) white: Color,
    pub(crate) panel_radius: f32,
    pub(crate) control_radius: f32,
    pub(crate) title_font_size: f32,
    pub(crate) footer_font_size: f32,
}

impl ModalVisual {
    // 关闭态只解析触发器实际使用的主题角色。
    pub(crate) fn resolve_trigger(&self, tokens: &dyn ThemeTokens) -> ResolvedModalTriggerVisual {
        ResolvedModalTriggerVisual {
            primary: self.palette.primary.resolve(tokens),
            primary_hover: self.palette.primary_hover.resolve(tokens),
            primary_active: self.palette.primary_active.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            radius: self.chrome.trigger_radius.resolve(tokens),
            font_size: self.typography.trigger.resolve(tokens),
        }
    }

    // 打开态按真实可见分支解析主题，避免隐藏关闭区和 footer 的虚调用。
    pub(crate) fn resolve_dialog(
        &self,
        tokens: &dyn ThemeTokens,
        has_close_control: bool,
        has_footer: bool,
    ) -> ResolvedModalDialogVisual {
        let has_controls = has_close_control || has_footer;
        ResolvedModalDialogVisual {
            mask: self.palette.mask.resolve(tokens),
            background: self.palette.panel_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: if has_close_control {
                self.palette.text_secondary.resolve(tokens)
            } else {
                Color::transparent()
            },
            fill_secondary: if has_controls {
                self.palette.fill_secondary.resolve(tokens)
            } else {
                Color::transparent()
            },
            fill_tertiary: if has_controls {
                self.palette.fill_tertiary.resolve(tokens)
            } else {
                Color::transparent()
            },
            primary: if has_footer {
                self.palette.primary.resolve(tokens)
            } else {
                Color::transparent()
            },
            primary_hover: if has_footer {
                self.palette.primary_hover.resolve(tokens)
            } else {
                Color::transparent()
            },
            primary_active: if has_footer {
                self.palette.primary_active.resolve(tokens)
            } else {
                Color::transparent()
            },
            white: if has_footer {
                self.palette.white.resolve(tokens)
            } else {
                Color::transparent()
            },
            panel_radius: self.chrome.panel_radius.resolve(tokens),
            control_radius: if has_controls {
                self.chrome.control_radius.resolve(tokens)
            } else {
                0.0
            },
            title_font_size: self.typography.title.resolve(tokens),
            footer_font_size: if has_footer {
                self.typography.footer.resolve(tokens)
            } else {
                0.0
            },
        }
    }
}

pub(crate) const fn modal_font_large() -> ModalFontRole {
    ModalFontRole::Large
}
pub(crate) const fn modal_font_fixed(value: f32) -> ModalFontRole {
    ModalFontRole::Fixed(value)
}
pub(crate) const fn modal_radius_normal() -> ModalRadiusRole {
    ModalRadiusRole::Normal
}
pub(crate) const fn modal_radius_small() -> ModalRadiusRole {
    ModalRadiusRole::Small
}
pub(crate) const fn modal_radius_large() -> ModalRadiusRole {
    ModalRadiusRole::Large
}
pub(crate) const fn modal_bg_mask() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgMask)
}
pub(crate) const fn modal_bg_container() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn modal_border_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BorderSecondary)
}
pub(crate) const fn modal_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn modal_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn modal_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn modal_primary_hover() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryHover)
}
pub(crate) const fn modal_primary_active() -> ColorValue {
    ColorValue::Palette(PaletteColor::PrimaryActive)
}
pub(crate) const fn modal_fill_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillSecondary)
}
pub(crate) const fn modal_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn modal_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
