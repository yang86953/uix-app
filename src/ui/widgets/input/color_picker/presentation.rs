//! ColorPicker 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存 UIX 声明的 24 个默认预设色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColorPickerPresetsVisual {
    color01: u32,
    color02: u32,
    color03: u32,
    color04: u32,
    color05: u32,
    color06: u32,
    color07: u32,
    color08: u32,
    color09: u32,
    color10: u32,
    color11: u32,
    color12: u32,
    color13: u32,
    color14: u32,
    color15: u32,
    color16: u32,
    color17: u32,
    color18: u32,
    color19: u32,
    color20: u32,
    color21: u32,
    color22: u32,
    color23: u32,
    color24: u32,
}

impl ColorPickerPresetsVisual {
    // 按 UIX 声明顺序返回默认 RGB24 色板。
    pub(crate) const fn values(self) -> [u32; 24] {
        [
            self.color01,
            self.color02,
            self.color03,
            self.color04,
            self.color05,
            self.color06,
            self.color07,
            self.color08,
            self.color09,
            self.color10,
            self.color11,
            self.color12,
            self.color13,
            self.color14,
            self.color15,
            self.color16,
            self.color17,
            self.color18,
            self.color19,
            self.color20,
            self.color21,
            self.color22,
            self.color23,
            self.color24,
        ]
    }
}

// 保存触发色块、颜色面板和棋盘格使用的全部静态几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorPickerLayoutVisual {
    pub(crate) panel_gap: f32,
    pub(crate) panel_columns: usize,
    pub(crate) panel_cell: f32,
    pub(crate) panel_padding: f32,
    pub(crate) fallback_panel_sides: f32,
    pub(crate) swatch_inset: f32,
    pub(crate) swatch_border_width: f32,
    pub(crate) focus_outset: f32,
    pub(crate) focus_border_width: f32,
    pub(crate) checker_tile: f32,
    pub(crate) checker_tile_min: f32,
    pub(crate) cell_inset: f32,
    pub(crate) cell_radius: f32,
    pub(crate) highlight_border_width: f32,
    pub(crate) selected_icon_size: f32,
}

// 保存面板进入与退出时序。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorPickerMotionVisual {
    pub(crate) enter_duration: f64,
    pub(crate) exit_duration: f64,
}

// 保存选中标记图标名称。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColorPickerIconsVisual {
    pub(crate) selected: &'static str,
}

// 保存面板描边、层级和两档圆角角色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorPickerChromeVisual {
    pub(crate) panel_border_width: f32,
    pub(crate) overlay_z: i32,
    swatch_radius: ColorPickerRadiusRole,
    panel_radius: ColorPickerRadiusRole,
}

// 保存 ColorPicker 使用的全部主题及固定颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ColorPickerPaletteVisual {
    popup_background: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    contrast_light: ColorValue,
    contrast_dark: ColorValue,
    checker_light: ColorValue,
    checker_dark: ColorValue,
}

// 保存主题圆角角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColorPickerRadiusRole {
    Base,
    Small,
}

impl ColorPickerRadiusRole {
    fn resolve(self, tokens: &dyn ThemeTokens) -> f32 {
        match self {
            Self::Base => tokens.border_radius(),
            Self::Small => tokens.border_radius_sm(),
        }
    }
}

// 全部 ColorPicker 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ColorPickerVisual {
    pub(crate) presets: ColorPickerPresetsVisual,
    pub(crate) layout: ColorPickerLayoutVisual,
    pub(crate) motion: ColorPickerMotionVisual,
    pub(crate) icons: ColorPickerIconsVisual,
    pub(crate) chrome: ColorPickerChromeVisual,
    palette: ColorPickerPaletteVisual,
}

crate::uix_items!("src/ui/widgets/input/color_picker/color_picker.uix");

// 保存每帧一次解析得到的主题颜色和圆角。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedColorPickerVisual {
    pub(crate) popup_background: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) contrast_light: Color,
    pub(crate) contrast_dark: Color,
    pub(crate) checker_light: Color,
    pub(crate) checker_dark: Color,
    pub(crate) swatch_radius: f32,
    pub(crate) panel_radius: f32,
}

impl ColorPickerVisual {
    // 同帧触发色块、棋盘格与颜色面板共享一次主题解析。
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedColorPickerVisual {
        ResolvedColorPickerVisual {
            popup_background: self.palette.popup_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            contrast_light: self.palette.contrast_light.resolve(tokens),
            contrast_dark: self.palette.contrast_dark.resolve(tokens),
            checker_light: self.palette.checker_light.resolve(tokens),
            checker_dark: self.palette.checker_dark.resolve(tokens),
            swatch_radius: self.chrome.swatch_radius.resolve(tokens),
            panel_radius: self.chrome.panel_radius.resolve(tokens),
        }
    }
}

pub(crate) const fn color_picker_radius_base() -> ColorPickerRadiusRole {
    ColorPickerRadiusRole::Base
}
pub(crate) const fn color_picker_radius_small() -> ColorPickerRadiusRole {
    ColorPickerRadiusRole::Small
}
pub(crate) const fn color_picker_popup_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn color_picker_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn color_picker_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn color_picker_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
pub(crate) const fn color_picker_black() -> ColorValue {
    ColorValue::Palette(PaletteColor::Black)
}
pub(crate) const fn color_picker_checker_dark() -> ColorValue {
    ColorValue::Custom(Color::from_rgb(0xD9, 0xD9, 0xD9))
}
