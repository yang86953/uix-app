//! Transfer 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存面板、搜索框、列表行、按钮和文本光标的静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransferLayoutVisual {
    pub(crate) natural_width: f32,
    pub(crate) natural_height: f32,
    pub(crate) button_column_width: f32,
    pub(crate) header_height: f32,
    pub(crate) search_height: f32,
    pub(crate) row_height: f32,
    pub(crate) min_pane_half_width: f32,
    pub(crate) search_horizontal_inset: f32,
    pub(crate) search_vertical_inset: f32,
    pub(crate) search_box_height: f32,
    pub(crate) text_horizontal_inset: f32,
    pub(crate) text_top_inset: f32,
    pub(crate) row_icon_inset: f32,
    pub(crate) row_icon_width: f32,
    pub(crate) row_text_inset: f32,
    pub(crate) button_horizontal_inset: f32,
    pub(crate) button_width: f32,
    pub(crate) button_height: f32,
    pub(crate) button_group_half_height: f32,
    pub(crate) button_vertical_gap: f32,
    pub(crate) cursor_char_width: f32,
    pub(crate) cursor_padding: f32,
    pub(crate) cursor_min_width: f32,
    pub(crate) cursor_max_width: f32,
    pub(crate) cursor_x_inset: f32,
    pub(crate) cursor_y: f32,
    pub(crate) cursor_width: f32,
    pub(crate) cursor_height: f32,
}

// 保存主题字号插值与图标字号。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransferTypographyVisual {
    pub(crate) item_midpoint_weight: f32,
    pub(crate) checkbox_icon_size: f32,
    pub(crate) arrow_icon_size: f32,
}

// 保存面板、活动行、按钮描边和圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransferChromeVisual {
    pub(crate) border_width: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) button_radius: f32,
    radius: TransferRadiusRole,
}

// 保存 Transfer 使用的图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransferIconsVisual {
    pub(crate) selected: &'static str,
    pub(crate) unselected: &'static str,
    pub(crate) move_right: &'static str,
    pub(crate) move_left: &'static str,
}

// 保存 Transfer 使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TransferPaletteVisual {
    background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_quaternary: ColorValue,
    primary: ColorValue,
    fill_tertiary: ColorValue,
    white: ColorValue,
}

// 保存主题小圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TransferRadiusRole {
    Small,
}

impl TransferRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 保存一次绘制内解析出的 Transfer 排版值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransferTypography {
    pub(crate) caption: f32,
    pub(crate) item: f32,
}

impl TransferTypography {
    // 兼容既有排版测试入口，插值权重仍由 UIX 唯一声明。
    pub(crate) fn resolve(tokens: &dyn ThemeTokens) -> Self {
        Self::resolve_with_visual(tokens, TRANSFER_VISUAL_REF.typography)
    }

    fn resolve_with_visual(tokens: &dyn ThemeTokens, visual: TransferTypographyVisual) -> Self {
        let caption = tokens.font_size_sm();
        let body = tokens.font_size();
        Self {
            caption,
            item: caption + (body - caption) * visual.item_midpoint_weight,
        }
    }
}

// 全部 Transfer 实例共享的完整 UIX 静态视觉表。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TransferVisual {
    pub(crate) layout: TransferLayoutVisual,
    typography: TransferTypographyVisual,
    pub(crate) chrome: TransferChromeVisual,
    pub(crate) icons: TransferIconsVisual,
    palette: TransferPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/transfer/transfer.uix");

// 保存左右面板一次解析共享的主题值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedTransferVisual {
    pub(crate) background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_quaternary: Color,
    pub(crate) primary: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) white: Color,
    pub(crate) radius: f32,
    pub(crate) typography: TransferTypography,
    pub(crate) checkbox_icon_size: f32,
    pub(crate) arrow_icon_size: f32,
}

impl TransferVisual {
    // 左右面板、按钮与列表在同一帧只解析一次主题角色。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedTransferVisual {
        ResolvedTransferVisual {
            background: self.palette.background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_quaternary: self.palette.text_quaternary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            radius: self.chrome.radius.resolve(tokens),
            typography: TransferTypography::resolve_with_visual(tokens, self.typography),
            checkbox_icon_size: self.typography.checkbox_icon_size,
            arrow_icon_size: self.typography.arrow_icon_size,
        }
    }
}

pub(crate) const fn transfer_radius_small() -> TransferRadiusRole {
    TransferRadiusRole::Small
}
pub(crate) const fn transfer_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn transfer_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn transfer_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn transfer_text_quaternary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextQuaternary)
}
pub(crate) const fn transfer_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn transfer_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
pub(crate) const fn transfer_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
