//! BarChart 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存 UIX 声明的默认尺寸与可由 Rust 调用方覆盖的初始视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BarChartDefaultsVisual {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) show_value: bool,
    pub(crate) bar_radius: f32,
    pub(crate) bar_gap: f32,
    pub(crate) category_gap: f32,
    pub(crate) padding: f32,
}

// 保存标题、图例、坐标轴和绘图区的静态布局比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BarChartLayoutVisual {
    pub(crate) title_height: f32,
    pub(crate) subtitle_height: f32,
    pub(crate) legend_row_height: f32,
    pub(crate) legend_side_ratio: f32,
    pub(crate) legend_side_min: f32,
    pub(crate) legend_side_max: f32,
    pub(crate) y_label_width: f32,
    pub(crate) y_label_width_ratio: f32,
    pub(crate) category_label_height: f32,
    pub(crate) value_label_height: f32,
    pub(crate) plot_bottom_gap: f32,
    pub(crate) grid_min_spacing: f32,
    pub(crate) grid_min_lines: usize,
    pub(crate) min_value_label_extent: f32,
    pub(crate) center_ratio: f32,
}

// 保存线宽、留白、提示框与交互叠层视觉。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BarChartChromeVisual {
    pub(crate) axis_stroke: f32,
    pub(crate) grid_stroke: f32,
    pub(crate) crosshair_stroke: f32,
    pub(crate) brush_alpha: u8,
    pub(crate) label_gap: f32,
    pub(crate) value_gap: f32,
    pub(crate) tooltip_offset: f32,
    pub(crate) tooltip_padding: f32,
    pub(crate) tooltip_edge_inset: f32,
    pub(crate) tooltip_border: f32,
}

// 保存标题、刻度、数值、分类、提示框和图例排版。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BarChartTypographyVisual {
    pub(crate) title: f32,
    pub(crate) subtitle: f32,
    pub(crate) grid: f32,
    pub(crate) value: f32,
    pub(crate) category: f32,
    pub(crate) tooltip: f32,
    pub(crate) legend: f32,
    pub(crate) series_separator: &'static str,
    pub(crate) single_series_legend: &'static str,
}

// 保存由 UIX 声明的主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct BarChartPaletteVisual {
    background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    elevated: ColorValue,
}

// 全部 BarChart 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct BarChartVisual {
    pub(crate) defaults: BarChartDefaultsVisual,
    pub(crate) layout: BarChartLayoutVisual,
    pub(crate) chrome: BarChartChromeVisual,
    pub(crate) typography: BarChartTypographyVisual,
    palette: BarChartPaletteVisual,
}

// 保存 BarChart 每帧只解析一次的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedBarChartVisual {
    pub(crate) background: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) elevated: Color,
}

impl BarChartVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedBarChartVisual {
        ResolvedBarChartVisual {
            background: self.palette.background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            elevated: self.palette.elevated.resolve(tokens),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn bar_chart_defaults_visual(
    width: f32,
    height: f32,
    show_value: bool,
    bar_radius: f32,
    bar_gap: f32,
    category_gap: f32,
    padding: f32,
) -> BarChartDefaultsVisual {
    BarChartDefaultsVisual {
        width,
        height,
        show_value,
        bar_radius,
        bar_gap,
        category_gap,
        padding,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn bar_chart_layout_visual(
    title_height: f32,
    subtitle_height: f32,
    legend_row_height: f32,
    legend_side_ratio: f32,
    legend_side_min: f32,
    legend_side_max: f32,
    y_label_width: f32,
    y_label_width_ratio: f32,
    category_label_height: f32,
    value_label_height: f32,
    plot_bottom_gap: f32,
    grid_min_spacing: f32,
    grid_min_lines: f32,
    min_value_label_extent: f32,
    center_ratio: f32,
) -> BarChartLayoutVisual {
    let grid_min_lines = grid_min_lines as usize;
    BarChartLayoutVisual {
        title_height,
        subtitle_height,
        legend_row_height,
        legend_side_ratio,
        legend_side_min,
        legend_side_max,
        y_label_width,
        y_label_width_ratio,
        category_label_height,
        value_label_height,
        plot_bottom_gap,
        grid_min_spacing,
        grid_min_lines: if grid_min_lines == 0 {
            1
        } else {
            grid_min_lines
        },
        min_value_label_extent,
        center_ratio,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn bar_chart_chrome_visual(
    axis_stroke: f32,
    grid_stroke: f32,
    crosshair_stroke: f32,
    brush_alpha: f32,
    label_gap: f32,
    value_gap: f32,
    tooltip_offset: f32,
    tooltip_padding: f32,
    tooltip_edge_inset: f32,
    tooltip_border: f32,
) -> BarChartChromeVisual {
    BarChartChromeVisual {
        axis_stroke,
        grid_stroke,
        crosshair_stroke,
        brush_alpha: brush_alpha as u8,
        label_gap,
        value_gap,
        tooltip_offset,
        tooltip_padding,
        tooltip_edge_inset,
        tooltip_border,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn bar_chart_typography_visual(
    title: f32,
    subtitle: f32,
    grid: f32,
    value: f32,
    category: f32,
    tooltip: f32,
    legend: f32,
    series_separator: &'static str,
    single_series_legend: &'static str,
) -> BarChartTypographyVisual {
    BarChartTypographyVisual {
        title,
        subtitle,
        grid,
        value,
        category,
        tooltip,
        legend,
        series_separator,
        single_series_legend,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn bar_chart_palette_visual(
    background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    elevated: ColorValue,
) -> BarChartPaletteVisual {
    BarChartPaletteVisual {
        background,
        text,
        text_secondary,
        border,
        primary,
        elevated,
    }
}

pub(crate) const fn bar_chart_visual(
    defaults: BarChartDefaultsVisual,
    layout: BarChartLayoutVisual,
    chrome: BarChartChromeVisual,
    typography: BarChartTypographyVisual,
    palette: BarChartPaletteVisual,
) -> BarChartVisual {
    BarChartVisual {
        defaults,
        layout,
        chrome,
        typography,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接书写的静态文案与主题角色。
pub(crate) const fn bar_chart_single_series_legend() -> &'static str {
    "数据"
}
pub(crate) const fn bar_chart_series_separator() -> &'static str {
    "  "
}
pub(crate) const fn bar_chart_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn bar_chart_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn bar_chart_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn bar_chart_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn bar_chart_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn bar_chart_elevated_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

pub(crate) static DEFAULT_BAR_CHART_VISUAL: BarChartVisual = bar_chart_visual(
    bar_chart_defaults_visual(300.0, 200.0, true, 2.0, 0.2, 0.2, 0.0),
    bar_chart_layout_visual(
        20.0, 16.0, 18.0, 0.24, 64.0, 120.0, 36.0, 0.35, 14.0, 14.0, 4.0, 30.0, 4.0, 10.0, 0.5,
    ),
    bar_chart_chrome_visual(1.0, 0.5, 1.0, 48.0, 4.0, 2.0, 12.0, 4.0, 4.0, 1.0),
    bar_chart_typography_visual(15.0, 11.0, 9.0, 10.0, 10.0, 10.0, 10.0, "  ", "数据"),
    bar_chart_palette_visual(
        ColorValue::Neutral(NeutralRole::BgContainer),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::Border),
        ColorValue::Palette(PaletteColor::Primary),
        ColorValue::Neutral(NeutralRole::BgElevated),
    ),
);

// 首次 UIX 构建固化声明值，全部 BarChart 实例共享一份视觉表。
pub(crate) static UIX_BAR_CHART_VISUAL: OnceLock<BarChartVisual> = OnceLock::new();
