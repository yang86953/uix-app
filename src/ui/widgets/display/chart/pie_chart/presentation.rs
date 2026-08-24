//! PieChart 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

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

#[allow(clippy::too_many_arguments)]
pub(crate) const fn pie_chart_defaults_visual(
    size: f32,
    hole_radius: f32,
    label_visible: bool,
    label_position: LabelPosition,
    legend: LegendPosition,
    padding: f32,
) -> PieChartDefaultsVisual {
    PieChartDefaultsVisual {
        size,
        hole_radius,
        label_visible,
        label_position,
        legend,
        padding,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn pie_chart_layout_visual(
    title_height: f32,
    subtitle_height: f32,
    chart_edge_inset: f32,
    legend_row_height: f32,
    legend_side_min_content: f32,
    legend_side_ratio: f32,
    legend_side_min: f32,
    legend_side_max: f32,
    legend_item_height: f32,
    legend_swatch_x: f32,
    legend_swatch_y: f32,
    legend_swatch_size: f32,
    legend_text_x: f32,
    legend_text_y: f32,
    center_ratio: f32,
) -> PieChartLayoutVisual {
    PieChartLayoutVisual {
        title_height,
        subtitle_height,
        chart_edge_inset,
        legend_row_height,
        legend_side_min_content,
        legend_side_ratio,
        legend_side_min,
        legend_side_max,
        legend_item_height,
        legend_swatch_x,
        legend_swatch_y,
        legend_swatch_size,
        legend_text_x,
        legend_text_y,
        center_ratio,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn pie_chart_label_visual(
    minimum_sweep_ratio: f32,
    minimum_percent: f32,
    font_ratio: f32,
    minimum_font_size: f32,
    centroid_factor: f32,
    outside_offset: f32,
    radial_font_extent_ratio: f32,
    line_height_ratio: f32,
    donut_gap: f32,
    center_font_ratio: f32,
) -> PieChartLabelVisual {
    PieChartLabelVisual {
        minimum_sweep_ratio,
        minimum_percent,
        font_ratio,
        minimum_font_size,
        centroid_factor,
        outside_offset,
        radial_font_extent_ratio,
        line_height_ratio,
        donut_gap,
        center_font_ratio,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn pie_chart_chrome_visual(
    hole_border: f32,
    crosshair_radius: f32,
    crosshair_stroke: f32,
    brush_alpha: f32,
    tooltip_offset: f32,
    tooltip_padding: f32,
    tooltip_edge_inset: f32,
    tooltip_border: f32,
) -> PieChartChromeVisual {
    PieChartChromeVisual {
        hole_border,
        crosshair_radius,
        crosshair_stroke,
        brush_alpha: brush_alpha as u8,
        tooltip_offset,
        tooltip_padding,
        tooltip_edge_inset,
        tooltip_border,
    }
}

pub(crate) const fn pie_chart_typography_visual(
    title: f32,
    subtitle: f32,
    legend: f32,
    tooltip: f32,
    label_separator: &'static str,
    legend_separator: &'static str,
    percent_suffix: &'static str,
) -> PieChartTypographyVisual {
    PieChartTypographyVisual {
        title,
        subtitle,
        legend,
        tooltip,
        label_separator,
        legend_separator,
        percent_suffix,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn pie_chart_palette_visual(
    background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    white: ColorValue,
    primary: ColorValue,
    elevated: ColorValue,
) -> PieChartPaletteVisual {
    PieChartPaletteVisual {
        background,
        text,
        text_secondary,
        border,
        white,
        primary,
        elevated,
    }
}

pub(crate) const fn pie_chart_visual(
    defaults: PieChartDefaultsVisual,
    layout: PieChartLayoutVisual,
    label: PieChartLabelVisual,
    chrome: PieChartChromeVisual,
    typography: PieChartTypographyVisual,
    palette: PieChartPaletteVisual,
) -> PieChartVisual {
    PieChartVisual {
        defaults,
        layout,
        label,
        chrome,
        typography,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接书写的枚举、静态文案与主题角色。
pub(crate) const fn pie_chart_inside_label_position() -> LabelPosition {
    LabelPosition::Inside
}
pub(crate) const fn pie_chart_right_legend_position() -> LegendPosition {
    LegendPosition::Right
}
pub(crate) const fn pie_chart_label_separator() -> &'static str {
    " "
}
pub(crate) const fn pie_chart_legend_separator() -> &'static str {
    "  "
}
pub(crate) const fn pie_chart_percent_suffix() -> &'static str {
    "%"
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

pub(crate) static DEFAULT_PIE_CHART_VISUAL: PieChartVisual = pie_chart_visual(
    pie_chart_defaults_visual(
        180.0,
        0.0,
        true,
        LabelPosition::Inside,
        LegendPosition::Right,
        0.0,
    ),
    pie_chart_layout_visual(
        20.0, 16.0, 4.0, 20.0, 120.0, 0.35, 70.0, 140.0, 18.0, 4.0, 5.0, 8.0, 16.0, 3.0, 0.5,
    ),
    pie_chart_label_visual(0.04, 3.0, 0.17, 8.0, 2.0 / 3.0, 10.0, 0.8, 1.6, 2.0, 0.6),
    pie_chart_chrome_visual(1.0, 4.0, 1.0, 48.0, 12.0, 4.0, 4.0, 1.0),
    pie_chart_typography_visual(15.0, 11.0, 10.0, 10.0, " ", "  ", "%"),
    pie_chart_palette_visual(
        ColorValue::Neutral(NeutralRole::BgContainer),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::Border),
        ColorValue::Palette(PaletteColor::White),
        ColorValue::Palette(PaletteColor::Primary),
        ColorValue::Neutral(NeutralRole::BgElevated),
    ),
);

// 首次 UIX 构建固化声明值，全部 PieChart 实例共享一份视觉表。
pub(crate) static UIX_PIE_CHART_VISUAL: OnceLock<PieChartVisual> = OnceLock::new();
