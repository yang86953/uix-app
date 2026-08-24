//! LineChart 的 UIX 静态视觉契约与主题解析。

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

// 同目录 UIX 生成折线图全部分组视觉、根记录及稳定借用。
crate::uix_items!("src/ui/widgets/display/chart/line_chart/line_chart.uix");

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

// 限制 UIX 声明的最少网格线数量，保持绘制除数为正。
pub(crate) const fn line_chart_grid_min_lines(value: usize) -> usize {
    if value == 0 { 1 } else { value }
}

// 限制 UIX 声明的平滑采样密度，避免曲线退化为零分段。
pub(crate) const fn line_chart_smooth_subdivisions(value: usize) -> usize {
    if value == 0 { 1 } else { value }
}

// 向 UIX 提供主题语义角色。
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
