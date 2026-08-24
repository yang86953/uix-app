//! 高级图表共享内核的 UIX 静态视觉契约与主题解析。

use crate::draw::Color;
use crate::ui::ThemeTokens;
use crate::ui::theme::NeutralRole;
use crate::ui::theme::style::{ColorValue, PaletteColor};

use super::{
    FunnelAlign, FunnelShape, GaugeType, LabelPosition, LegendPosition, PointStyle, RadarShape,
    RoseStyle,
};

// 保存构造器采用的视觉默认值；作者显式配置仍由 Rust 数据契约拥有。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AdvancedChartDefaultsVisual {
    pub(crate) width: f32,
    pub(crate) height: f32,
    pub(crate) padding: f32,
    pub(crate) legend: LegendPosition,
    pub(crate) bar_gap: f32,
    pub(crate) category_gap: f32,
    pub(crate) rose_style: RoseStyle,
    pub(crate) start_angle: f32,
    pub(crate) end_angle: f32,
    pub(crate) label_visible: bool,
    pub(crate) label_position: LabelPosition,
    pub(crate) bubble_scale: f32,
    pub(crate) point_size: f32,
    pub(crate) point_style: PointStyle,
    pub(crate) grid_levels: usize,
    pub(crate) fill_opacity: f32,
    pub(crate) heatmap_color_min: Color,
    pub(crate) heatmap_color_max: Color,
    pub(crate) cell_size: f32,
    pub(crate) cell_gap: f32,
    pub(crate) funnel_align: FunnelAlign,
    pub(crate) funnel_shape: FunnelShape,
    pub(crate) funnel_gap: f32,
    pub(crate) treemap_gap: f32,
    pub(crate) radar_shape: RadarShape,
    pub(crate) gauge_min: f32,
    pub(crate) gauge_max: f32,
    pub(crate) gauge_type: GaugeType,
    pub(crate) pointer_width: f32,
    pub(crate) zoom: f32,
}

// 保存共享画布、图例、坐标轴、提示与交互层几何。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AdvancedChartLayoutVisual {
    pub(crate) title_height: f32,
    pub(crate) subtitle_height: f32,
    pub(crate) axis_title_height: f32,
    pub(crate) legend_row_height: f32,
    pub(crate) legend_side_ratio: f32,
    pub(crate) legend_side_min_width: f32,
    pub(crate) legend_side_max_width: f32,
    pub(crate) legend_swatch_x: f32,
    pub(crate) legend_swatch_y: f32,
    pub(crate) legend_swatch_size: f32,
    pub(crate) legend_text_x: f32,
    pub(crate) legend_text_y: f32,
    pub(crate) tooltip_pointer_offset: f32,
    pub(crate) tooltip_edge_inset: f32,
    pub(crate) tooltip_padding: f32,
    pub(crate) reference_label_x: f32,
    pub(crate) reference_label_y: f32,
    pub(crate) outside_label_gap: f32,
    pub(crate) category_label_gap: f32,
    pub(crate) category_label_height: f32,
    pub(crate) radar_radius_ratio: f32,
    pub(crate) radar_label_offset: f32,
    pub(crate) heatmap_left_label_width: f32,
    pub(crate) heatmap_bottom_label_height: f32,
    pub(crate) heatmap_label_gap: f32,
    pub(crate) heatmap_month_offset: f32,
    pub(crate) funnel_min_ratio: f32,
    pub(crate) waterfall_bar_ratio: f32,
    pub(crate) waterfall_bar_inset_ratio: f32,
    pub(crate) treemap_label_x: f32,
    pub(crate) treemap_label_y: f32,
    pub(crate) treemap_inner_x: f32,
    pub(crate) treemap_inner_y: f32,
    pub(crate) treemap_inner_width_reduction: f32,
    pub(crate) treemap_inner_height_reduction: f32,
    pub(crate) gauge_center_y_ratio: f32,
    pub(crate) gauge_radius_ratio: f32,
    pub(crate) gauge_ring_inner_ratio: f32,
    pub(crate) gauge_pointer_length_ratio: f32,
    pub(crate) gauge_pointer_hub_ratio: f32,
    pub(crate) gauge_label_y_offset: f32,
    pub(crate) gauge_label_height: f32,
    pub(crate) bubble_radius_ratio: f32,
    pub(crate) bubble_radius_max: f32,
    pub(crate) bubble_radius_min: f32,
    pub(crate) scatter_radius_min: f32,
    pub(crate) scatter_radius_max: f32,
}

// 保存全部固定文字层级。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AdvancedChartTypographyVisual {
    pub(crate) title: f32,
    pub(crate) subtitle: f32,
    pub(crate) body: f32,
    pub(crate) caption: f32,
    pub(crate) treemap_leaf: f32,
    pub(crate) gauge_value: f32,
}

// 保存共享描边、虚线分段与交互透明度。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AdvancedChartChromeVisual {
    pub(crate) border_width: f32,
    pub(crate) series_width: f32,
    pub(crate) radar_outline_width: f32,
    pub(crate) reference_segments: usize,
    pub(crate) combo_dash_segments: usize,
    pub(crate) smooth_subdivisions: usize,
    pub(crate) brush_alpha: u8,
}

// 保存高级图表使用的全部主题颜色角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct AdvancedChartPaletteVisual {
    background: ColorValue,
    elevated_background: ColorValue,
    border: ColorValue,
    text: ColorValue,
    text_secondary: ColorValue,
    primary: ColorValue,
    warning: ColorValue,
    white: ColorValue,
    success: ColorValue,
    error: ColorValue,
    fill_tertiary: ColorValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct AdvancedChartVisual {
    pub(crate) defaults: AdvancedChartDefaultsVisual,
    pub(crate) layout: AdvancedChartLayoutVisual,
    pub(crate) typography: AdvancedChartTypographyVisual,
    pub(crate) chrome: AdvancedChartChromeVisual,
    palette: AdvancedChartPaletteVisual,
}

crate::uix_items!("src/ui/widgets/display/chart/advanced/advanced.uix");

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ResolvedAdvancedChartVisual {
    pub(crate) background: Color,
    pub(crate) elevated_background: Color,
    pub(crate) border: Color,
    pub(crate) text: Color,
    pub(crate) text_secondary: Color,
    pub(crate) primary: Color,
    pub(crate) warning: Color,
    pub(crate) white: Color,
    pub(crate) success: Color,
    pub(crate) error: Color,
    pub(crate) fill_tertiary: Color,
    pub(crate) empty_font_size: f32,
}

impl AdvancedChartVisual {
    pub(crate) fn resolve(&self, tokens: &dyn ThemeTokens) -> ResolvedAdvancedChartVisual {
        ResolvedAdvancedChartVisual {
            background: self.palette.background.resolve(tokens),
            elevated_background: self.palette.elevated_background.resolve(tokens),
            border: self.palette.border.resolve(tokens),
            text: self.palette.text.resolve(tokens),
            text_secondary: self.palette.text_secondary.resolve(tokens),
            primary: self.palette.primary.resolve(tokens),
            warning: self.palette.warning.resolve(tokens),
            white: self.palette.white.resolve(tokens),
            success: self.palette.success.resolve(tokens),
            error: self.palette.error.resolve(tokens),
            fill_tertiary: self.palette.fill_tertiary.resolve(tokens),
            empty_font_size: tokens.font_size_sm(),
        }
    }
}

pub(crate) const fn advanced_legend_none() -> LegendPosition {
    LegendPosition::None
}
pub(crate) const fn advanced_rose_radius() -> RoseStyle {
    RoseStyle::Radius
}
pub(crate) const fn advanced_label_outside() -> LabelPosition {
    LabelPosition::Outside
}
pub(crate) const fn advanced_point_circle() -> PointStyle {
    PointStyle::Circle
}
pub(crate) const fn advanced_funnel_center() -> FunnelAlign {
    FunnelAlign::Center
}
pub(crate) const fn advanced_funnel_normal() -> FunnelShape {
    FunnelShape::Normal
}
pub(crate) const fn advanced_radar_polygon() -> RadarShape {
    RadarShape::Polygon
}
pub(crate) const fn advanced_gauge_dashboard() -> GaugeType {
    GaugeType::Dashboard
}
pub(crate) const fn advanced_heatmap_min() -> Color {
    Color::from_rgba(247, 251, 255, 255)
}
pub(crate) const fn advanced_heatmap_max() -> Color {
    Color::from_rgba(8, 48, 107, 255)
}
pub(crate) const fn advanced_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgContainer)
}
pub(crate) const fn advanced_elevated_background() -> ColorValue {
    ColorValue::Neutral(NeutralRole::BgElevated)
}
pub(crate) const fn advanced_border() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Border)
}
pub(crate) const fn advanced_text() -> ColorValue {
    ColorValue::Neutral(NeutralRole::Text)
}
pub(crate) const fn advanced_text_secondary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::TextSecondary)
}
pub(crate) const fn advanced_primary() -> ColorValue {
    ColorValue::Palette(PaletteColor::Primary)
}
pub(crate) const fn advanced_warning() -> ColorValue {
    ColorValue::Palette(PaletteColor::Warning)
}
pub(crate) const fn advanced_white() -> ColorValue {
    ColorValue::Palette(PaletteColor::White)
}
pub(crate) const fn advanced_success() -> ColorValue {
    ColorValue::Palette(PaletteColor::Success)
}
pub(crate) const fn advanced_error() -> ColorValue {
    ColorValue::Palette(PaletteColor::Error)
}
pub(crate) const fn advanced_fill_tertiary() -> ColorValue {
    ColorValue::Neutral(NeutralRole::FillTertiary)
}
