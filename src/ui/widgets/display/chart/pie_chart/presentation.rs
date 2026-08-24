//! PieChart 的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

use super::super::advanced::{LabelPosition, LegendPosition};

// 保存 UIX 声明的固有尺寸与可由 Rust 调用方覆盖的初始视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieChartDefaultsVisual {
    pub(crate) size: f32,
    pub(crate) hole_radius: f32,
    pub(crate) label_visible: bool,
    pub(crate) label_position: LabelPosition,
    pub(crate) legend: LegendPosition,
    pub(crate) padding: f32,
}

// 保存标题、绘图区与横纵图例的静态布局。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieChartLayoutVisual {
    pub(crate) title_height: f32,
    pub(crate) subtitle_height: f32,
    pub(crate) chart_edge_inset: f32,
    pub(crate) legend_row_height: f32,
    pub(crate) legend_side_min_content: f32,
    pub(crate) legend_side_ratio: f32,
    pub(crate) legend_side_min: f32,
    pub(crate) legend_side_max: f32,
    pub(crate) legend_item_height: f32,
    pub(crate) legend_swatch_x: f32,
    pub(crate) legend_swatch_y: f32,
    pub(crate) legend_swatch_size: f32,
    pub(crate) legend_text_x: f32,
    pub(crate) legend_text_y: f32,
    pub(crate) center_ratio: f32,
}

// 保存扇区标签、环图安全间隔与中心汇总文字的静态比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieChartLabelVisual {
    pub(crate) minimum_sweep_ratio: f32,
    pub(crate) minimum_percent: f32,
    pub(crate) font_ratio: f32,
    pub(crate) minimum_font_size: f32,
    pub(crate) centroid_factor: f32,
    pub(crate) outside_offset: f32,
    pub(crate) radial_font_extent_ratio: f32,
    pub(crate) line_height_ratio: f32,
    pub(crate) donut_gap: f32,
    pub(crate) center_font_ratio: f32,
}

// 保存内环、交互叠层与提示框的静态绘制参数。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieChartChromeVisual {
    pub(crate) hole_border: f32,
    pub(crate) crosshair_radius: f32,
    pub(crate) crosshair_stroke: f32,
    pub(crate) brush_alpha: u8,
    pub(crate) tooltip_offset: f32,
    pub(crate) tooltip_padding: f32,
    pub(crate) tooltip_edge_inset: f32,
    pub(crate) tooltip_border: f32,
}

// 保存标题、图例、提示框字号与图例文本分隔符。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieChartTypographyVisual {
    pub(crate) title: f32,
    pub(crate) subtitle: f32,
    pub(crate) legend: f32,
    pub(crate) tooltip: f32,
    pub(crate) label_separator: &'static str,
    pub(crate) legend_separator: &'static str,
    pub(crate) percent_suffix: &'static str,
}

// 保存由 UIX 声明的主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PieChartPaletteVisual {
    background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    white: ColorValue,
    primary: ColorValue,
    elevated: ColorValue,
}

// 全部 PieChart 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PieChartVisual {
    pub(crate) defaults: PieChartDefaultsVisual,
    pub(crate) layout: PieChartLayoutVisual,
    pub(crate) label: PieChartLabelVisual,
    pub(crate) chrome: PieChartChromeVisual,
    pub(crate) typography: PieChartTypographyVisual,
    palette: PieChartPaletteVisual,
}

// 同目录 UIX 生成饼图全部分组视觉、根记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/chart/pie_chart/pie_chart.uix");

// 保存 PieChart 每帧只解析一次的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedPieChartVisual {
    pub(crate) background: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) border: Color,
    pub(crate) white: Color,
    pub(crate) primary: Color,
    pub(crate) elevated: Color,
}

impl PieChartVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedPieChartVisual {
        ResolvedPieChartVisual {
            background: self.palette.background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            elevated: self.palette.elevated.resolve(tokens),
        }
    }
}

// 向 UIX 提供枚举与主题语义角色。
pub(crate) const fn pie_chart_inside_label_position() -> LabelPosition {
    LabelPosition::Inside
}
pub(crate) const fn pie_chart_right_legend_position() -> LegendPosition {
    LegendPosition::Right
}
pub(crate) const fn pie_chart_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn pie_chart_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn pie_chart_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn pie_chart_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn pie_chart_white_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
pub(crate) const fn pie_chart_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn pie_chart_elevated_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
