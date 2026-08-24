//! LineChart 的 UIX 静态视觉契约与主题解析。

use std::sync::OnceLock;

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

// 保存 UIX 声明的默认尺寸与可由 Rust 调用方覆盖的初始视觉值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineChartDefaultsVisual {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) show_grid: bool,
    pub(crate) show_dots: bool,
    pub(crate) line_width: f32,
    pub(crate) dot_radius: f32,
    pub(crate) padding: f32,
}

// 保存标题、图例、坐标轴与分类标签的静态布局比例。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineChartLayoutVisual {
    pub(crate) title_height: f32,
    pub(crate) subtitle_height: f32,
    pub(crate) legend_row_height: f32,
    pub(crate) legend_side_ratio: f32,
    pub(crate) legend_side_min: f32,
    pub(crate) legend_side_max: f32,
    pub(crate) y_label_width: f32,
    pub(crate) y_label_width_ratio: f32,
    pub(crate) category_label_height: f32,
    pub(crate) plot_bottom_reserve: f32,
    pub(crate) category_label_gap: f32,
    pub(crate) grid_min_spacing: f32,
    pub(crate) grid_min_lines: usize,
    pub(crate) center_ratio: f32,
}

// 保存线宽、提示框、交互叠层与数据点内部留白。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineChartChromeVisual {
    pub(crate) axis_stroke: f32,
    pub(crate) grid_stroke: f32,
    pub(crate) crosshair_stroke: f32,
    pub(crate) brush_alpha: u8,
    pub(crate) label_gap: f32,
    pub(crate) tooltip_offset: f32,
    pub(crate) tooltip_padding: f32,
    pub(crate) tooltip_edge_inset: f32,
    pub(crate) tooltip_border: f32,
    pub(crate) dot_inner_inset: f32,
    pub(crate) dot_inner_min_radius: f32,
}

// 保存标题、刻度、分类、提示框和图例排版。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineChartTypographyVisual {
    pub(crate) title: f32,
    pub(crate) subtitle: f32,
    pub(crate) grid: f32,
    pub(crate) category: f32,
    pub(crate) tooltip: f32,
    pub(crate) legend: f32,
    pub(crate) series_separator: &'static str,
    pub(crate) single_series_legend: &'static str,
}

// 保存平滑曲线的静态采样密度。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineChartMotionVisual {
    pub(crate) smooth_subdivisions: usize,
}

// 保存由 UIX 声明的主题语义角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LineChartPaletteVisual {
    background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    success: ColorValue,
    warning: ColorValue,
    error: ColorValue,
    elevated: ColorValue,
}

// 全部 LineChart 实例共享的完整静态视觉配置。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LineChartVisual {
    pub(crate) defaults: LineChartDefaultsVisual,
    pub(crate) layout: LineChartLayoutVisual,
    pub(crate) chrome: LineChartChromeVisual,
    pub(crate) typography: LineChartTypographyVisual,
    pub(crate) motion: LineChartMotionVisual,
    palette: LineChartPaletteVisual,
}

// 保存 LineChart 每帧只解析一次的主题颜色。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedLineChartVisual {
    pub(crate) background: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) border: Color,
    pub(crate) primary: Color,
    pub(crate) success: Color,
    pub(crate) warning: Color,
    pub(crate) error: Color,
    pub(crate) elevated: Color,
}

impl LineChartVisual {
    pub(crate) fn resolve(self, tokens: &dyn ThemeTokens) -> ResolvedLineChartVisual {
        ResolvedLineChartVisual {
            background: self.palette.background.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            warning: self.palette.warning.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            elevated: self.palette.elevated.resolve(tokens),
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn line_chart_defaults_visual(
    width: f32,
    height: f32,
    show_grid: bool,
    show_dots: bool,
    line_width: f32,
    dot_radius: f32,
    padding: f32,
) -> LineChartDefaultsVisual {
    LineChartDefaultsVisual {
        width,
        height,
        show_grid,
        show_dots,
        line_width,
        dot_radius,
        padding,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn line_chart_layout_visual(
    title_height: f32,
    subtitle_height: f32,
    legend_row_height: f32,
    legend_side_ratio: f32,
    legend_side_min: f32,
    legend_side_max: f32,
    y_label_width: f32,
    y_label_width_ratio: f32,
    category_label_height: f32,
    plot_bottom_reserve: f32,
    category_label_gap: f32,
    grid_min_spacing: f32,
    grid_min_lines: f32,
    center_ratio: f32,
) -> LineChartLayoutVisual {
    let grid_min_lines = grid_min_lines as usize;
    LineChartLayoutVisual {
        title_height,
        subtitle_height,
        legend_row_height,
        legend_side_ratio,
        legend_side_min,
        legend_side_max,
        y_label_width,
        y_label_width_ratio,
        category_label_height,
        plot_bottom_reserve,
        category_label_gap,
        grid_min_spacing,
        grid_min_lines: if grid_min_lines == 0 {
            1
        } else {
            grid_min_lines
        },
        center_ratio,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn line_chart_chrome_visual(
    axis_stroke: f32,
    grid_stroke: f32,
    crosshair_stroke: f32,
    brush_alpha: f32,
    label_gap: f32,
    tooltip_offset: f32,
    tooltip_padding: f32,
    tooltip_edge_inset: f32,
    tooltip_border: f32,
    dot_inner_inset: f32,
    dot_inner_min_radius: f32,
) -> LineChartChromeVisual {
    LineChartChromeVisual {
        axis_stroke,
        grid_stroke,
        crosshair_stroke,
        brush_alpha: brush_alpha as u8,
        label_gap,
        tooltip_offset,
        tooltip_padding,
        tooltip_edge_inset,
        tooltip_border,
        dot_inner_inset,
        dot_inner_min_radius,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn line_chart_typography_visual(
    title: f32,
    subtitle: f32,
    grid: f32,
    category: f32,
    tooltip: f32,
    legend: f32,
    series_separator: &'static str,
    single_series_legend: &'static str,
) -> LineChartTypographyVisual {
    LineChartTypographyVisual {
        title,
        subtitle,
        grid,
        category,
        tooltip,
        legend,
        series_separator,
        single_series_legend,
    }
}

pub(crate) const fn line_chart_motion_visual(smooth_subdivisions: f32) -> LineChartMotionVisual {
    let smooth_subdivisions = smooth_subdivisions as usize;
    LineChartMotionVisual {
        smooth_subdivisions: if smooth_subdivisions == 0 {
            1
        } else {
            smooth_subdivisions
        },
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn line_chart_palette_visual(
    background: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    border: ColorValue,
    primary: ColorValue,
    success: ColorValue,
    warning: ColorValue,
    error: ColorValue,
    elevated: ColorValue,
) -> LineChartPaletteVisual {
    LineChartPaletteVisual {
        background,
        text,
        text_secondary,
        border,
        primary,
        success,
        warning,
        error,
        elevated,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) const fn line_chart_visual(
    defaults: LineChartDefaultsVisual,
    layout: LineChartLayoutVisual,
    chrome: LineChartChromeVisual,
    typography: LineChartTypographyVisual,
    motion: LineChartMotionVisual,
    palette: LineChartPaletteVisual,
) -> LineChartVisual {
    LineChartVisual {
        defaults,
        layout,
        chrome,
        typography,
        motion,
        palette,
    }
}

// 向 UIX 提供受限表达式不能直接书写的静态文案与主题角色。
pub(crate) const fn line_chart_series_separator() -> &'static str {
    "  "
}
pub(crate) const fn line_chart_single_series_legend() -> &'static str {
    "数据"
}
pub(crate) const fn line_chart_background_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn line_chart_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn line_chart_secondary_text_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn line_chart_border_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn line_chart_primary_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn line_chart_success_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
pub(crate) const fn line_chart_warning_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn line_chart_error_color() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(crate) const fn line_chart_elevated_color() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}

pub(crate) static DEFAULT_LINE_CHART_VISUAL: LineChartVisual = line_chart_visual(
    line_chart_defaults_visual(300.0, 200.0, true, true, 2.0, 3.0, 0.0),
    line_chart_layout_visual(
        20.0, 16.0, 18.0, 0.24, 64.0, 120.0, 36.0, 0.35, 12.0, 14.0, 2.0, 30.0, 4.0, 0.5,
    ),
    line_chart_chrome_visual(1.0, 0.5, 1.0, 48.0, 4.0, 12.0, 4.0, 4.0, 1.0, 1.5, 0.5),
    line_chart_typography_visual(15.0, 11.0, 9.0, 10.0, 10.0, 10.0, "  ", "数据"),
    line_chart_motion_visual(8.0),
    line_chart_palette_visual(
        ColorValue::Neutral(NeutralRole::BgContainer),
        ColorValue::Neutral(NeutralRole::Text),
        ColorValue::Neutral(NeutralRole::TextSecondary),
        ColorValue::Neutral(NeutralRole::Border),
        ColorValue::Palette(PaletteColor::Primary),
        ColorValue::Palette(PaletteColor::Success),
        ColorValue::Palette(PaletteColor::Warning),
        ColorValue::Palette(PaletteColor::Error),
        ColorValue::Neutral(NeutralRole::BgElevated),
    ),
);

// 首次 UIX 构建固化声明值，全部 LineChart 实例共享一份视觉表。
pub(crate) static UIX_LINE_CHART_VISUAL: OnceLock<LineChartVisual> = OnceLock::new();
