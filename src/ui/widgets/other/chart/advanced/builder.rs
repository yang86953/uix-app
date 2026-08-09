use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use crate::draw::Color;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    BrushConfig, BubbleData, ChartKind, ChartPayload, ChartPlaceholder, ChartSeries, ChartType,
    ComboSeries, FunnelAlign, FunnelData, FunnelShape, GaugeRange, GaugeType, HeatmapCell,
    InteractionConfig, LabelPosition, LegendPosition, LineStyle, PointStyle, RadarAxis,
    RadarData, RadarShape, RoseStyle, ScatterData, TooltipConfig, TreemapNode, WaterfallData,
    palette_color,
};


impl ChartPlaceholder {

    pub fn new() -> Self {
        Self {
            width: 300.0,
            height: 200.0,
            kind: ChartKind::Generic,
            payload: ChartPayload::Empty,
            background: None,
            padding: 16.0,
            legend: LegendPosition::None,
            title: String::new(),
            subtitle: String::new(),
            responsive: false,
            grouped: false,
            stacked: false,
            horizontal: false,
            smooth: false,
            step: false,
            bar_gap: 0.1,
            category_gap: 0.2,
            rose: false,
            rose_style: RoseStyle::Radius,
            start_angle: 0.0,
            end_angle: 360.0,
            total: None,
            label_visible: true,
            label_position: LabelPosition::Outside,
            x_axis: String::new(),
            y_axis: String::new(),
            y_axis_right: String::new(),
            bubble_scale: 1.0,
            point_size: 5.0,
            point_style: PointStyle::Circle,
            grid_levels: 5,
            fill_opacity: 0.25,
            color_min: Color::from_rgba(247, 251, 255, 255),
            color_max: Color::from_rgba(8, 48, 107, 255),
            calendar_mode: false,
            year: 0,
            cell_size: 14.0,
            cell_gap: 2.0,
            show_values: false,
            show_conversion_rate: false,
            funnel_align: FunnelAlign::Center,
            funnel_shape: FunnelShape::Normal,
            funnel_gap: 4.0,
            treemap_gap: 4.0,
            bar_series: Vec::new(),
            line_series: Vec::new(),
            combo_series: Vec::new(),
            scatter_series: Vec::new(),
            radar_axes: Vec::new(),
            radar_series: Vec::new(),
            radar_shape: RadarShape::Polygon,
            gauge_ranges: Vec::new(),
            gauge_value: 0.0,
            gauge_min: 0.0,
            gauge_max: 100.0,
            gauge_type: GaugeType::Dashboard,
            pointer_width: 3.0,
            pointer_color: None,
            value_format: None,
            interaction: None,
            brush_config: None,
            tooltip_config: None,
            x_labels: Vec::new(),
            y_labels: Vec::new(),
            color_stops: Vec::new(),
            reference_lines: Vec::new(),
            animation_config: None,
            animation_player: None,
            animation_dirty: Cell::new(false),
            last_frame: Cell::new(None),
            hovered_pos: Cell::new(None),
            tooltip_pos: Cell::new(None),
            brush_start: Cell::new(None),
            pan_start: Cell::new(None),
            pan_origin: Cell::new(0.0),
            pan_offset: Cell::new(0.0),
            zoom: Cell::new(1.0),
        }
    }

    pub(crate) fn new_with_kind(kind: ChartKind) -> Self {
        let mut chart = Self::new();
        chart.kind = kind;
        chart
    }

    pub fn data<T: 'static>(mut self, data: T) -> Self {
        let mut any: Box<dyn Any> = Box::new(data);
        macro_rules! take_payload {
            ($ty:ty, $payload:expr_2021, $kind:expr_2021) => {
                match any.downcast::<$ty>() {
                    Ok(value) => {
                        self.payload = $payload(*value);
                        self.kind = $kind;
                        return self;
                    }
                    Err(rest) => any = rest,
                }
            };
        }
        take_payload!(Vec<BarData>, ChartPayload::Bars, ChartKind::Bar);
        match any.downcast::<Vec<LineData>>() {
            Ok(value) => {
                self.payload = ChartPayload::Lines(*value);
                if self.kind != ChartKind::Area {
                    self.kind = ChartKind::Line;
                }
                return self;
            }
            Err(rest) => any = rest,
        }
        take_payload!(Vec<ScatterData>, ChartPayload::Scatter, ChartKind::Scatter);
        take_payload!(Vec<BubbleData>, ChartPayload::Bubble, ChartKind::Scatter);
        take_payload!(Vec<HeatmapCell>, ChartPayload::Heatmap, ChartKind::Heatmap);
        take_payload!(Vec<FunnelData>, ChartPayload::Funnel, ChartKind::Funnel);
        take_payload!(
            Vec<WaterfallData>,
            ChartPayload::Waterfall,
            ChartKind::Waterfall
        );
        take_payload!(Vec<TreemapNode>, ChartPayload::Treemap, ChartKind::Treemap);
        drop(any);
        self.payload = ChartPayload::Empty;
        self
    }
    pub fn series<T: 'static>(mut self, series: T) -> Self {
        let mut any: Box<dyn Any> = Box::new(series);
        any = match any.downcast::<Vec<ChartSeries<Vec<BarData>>>>() {
            Ok(value) => {
                self.bar_series = *value;
                self.payload = ChartPayload::BarSeries(self.bar_series.clone());
                self.kind = ChartKind::Bar;
                return self;
            }
            Err(rest) => rest,
        };
        any = match any.downcast::<Vec<ChartSeries<Vec<LineData>>>>() {
            Ok(value) => {
                self.line_series = *value;
                self.payload = ChartPayload::LineSeries(self.line_series.clone());
                if self.kind == ChartKind::Generic {
                    self.kind = ChartKind::Line;
                }
                return self;
            }
            Err(rest) => rest,
        };
        any = match any.downcast::<Vec<ChartSeries<Vec<RadarData>>>>() {
            Ok(value) => {
                self.radar_series = *value;
                self.kind = ChartKind::Radar;
                return self;
            }
            Err(rest) => rest,
        };
        any = match any.downcast::<Vec<ChartSeries<Vec<ScatterData>>>>() {
            Ok(value) => {
                self.scatter_series = *value;
                self.payload = ChartPayload::ScatterSeries(self.scatter_series.clone());
                self.kind = ChartKind::Scatter;
                return self;
            }
            Err(rest) => rest,
        };
        if let Ok(value) = any.downcast::<Vec<ComboSeries<Vec<LineData>>>>() {
            self.combo_series = *value;
            self.bar_series.clear();
            self.line_series.clear();
            for (index, series) in self.combo_series.iter().enumerate() {
                match series.chart_type {
                    ChartType::Bar => self.bar_series.push(ChartSeries::new(
                        series.name.clone(),
                        series
                            .data
                            .iter()
                            .map(|item| BarData {
                                label: item.label.clone(),
                                value: item.value,
                                color: palette_color(index),
                            })
                            .collect(),
                    )),
                    ChartType::Line | ChartType::Area => self
                        .line_series
                        .push(ChartSeries::new(series.name.clone(), series.data.clone())),
                }
            }
            self.kind = ChartKind::Combo;
        }
        self
    }
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(0.0);
        self
    }
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(0.0);
        self
    }
    pub fn size(mut self, size: f32) -> Self {
        self.width = size.max(0.0);
        self.height = size.max(0.0);
        self
    }
    pub fn grouped(mut self, value: bool) -> Self {
        self.grouped = value;
        self
    }
    pub fn stacked(mut self, value: bool) -> Self {
        self.stacked = value;
        self
    }
    pub fn horizontal(mut self, value: bool) -> Self {
        self.horizontal = value;
        self
    }
    pub fn bar_gap(mut self, value: f32) -> Self {
        self.bar_gap = if value.is_finite() {
            value.clamp(0.0, 0.9)
        } else {
            0.1
        };
        self
    }
    pub fn category_gap(mut self, value: f32) -> Self {
        self.category_gap = if value.is_finite() {
            value.clamp(0.0, 0.9)
        } else {
            0.2
        };
        self
    }
    pub fn legend(mut self, value: LegendPosition) -> Self {
        self.legend = value;
        self
    }
    pub fn smooth(mut self, value: bool) -> Self {
        self.smooth = value;
        self
    }
    pub fn step(mut self, value: bool) -> Self {
        self.step = value;
        self
    }
    pub fn rose(mut self, value: bool) -> Self {
        self.rose = value;
        if value {
            self.kind = ChartKind::Bar;
        }
        self
    }
    pub fn rose_style(mut self, value: RoseStyle) -> Self {
        self.rose_style = value;
        self
    }
    pub fn start_angle(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.start_angle = value;
        }
        self
    }
    pub fn end_angle(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.end_angle = value;
        }
        self
    }
    pub fn total(mut self, value: f32) -> Self {
        self.total = value.is_finite().then_some(value.max(0.0));
        self
    }
    pub fn label_visible(mut self, value: bool) -> Self {
        self.label_visible = value;
        self
    }
    pub fn label_position(mut self, value: LabelPosition) -> Self {
        self.label_position = value;
        self
    }
    pub fn x_axis(mut self, value: impl Into<String>) -> Self {
        self.x_axis = value.into();
        self
    }
    pub fn y_axis(mut self, value: impl Into<String>) -> Self {
        self.y_axis = value.into();
        self
    }
    pub fn bubble_scale(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.bubble_scale = value.max(0.0);
        }
        self
    }
    pub fn point_size(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.point_size = value.max(0.0);
        }
        self
    }
    pub fn point_style(mut self, value: PointStyle) -> Self {
        self.point_style = value;
        self
    }
    pub fn axes<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(axes) = any.downcast::<Vec<RadarAxis>>() {
            self.radar_axes = *axes;
            self.kind = ChartKind::Radar;
        }
        self
    }
    pub fn shape<T: 'static>(mut self, value: T) -> Self {
        let mut any: Box<dyn Any> = Box::new(value);
        any = match any.downcast::<RadarShape>() {
            Ok(shape) => {
                self.kind = ChartKind::Radar;
                self.radar_shape = *shape;
                return self;
            }
            Err(rest) => rest,
        };
        if let Ok(shape) = any.downcast::<FunnelShape>() {
            self.funnel_shape = *shape;
            self.kind = ChartKind::Funnel;
        }
        self
    }
    pub fn grid_levels(mut self, value: usize) -> Self {
        self.grid_levels = value.clamp(1, 64);
        self
    }
    pub fn fill_opacity(mut self, value: f32) -> Self {
        self.fill_opacity = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.25
        };
        if matches!(self.kind, ChartKind::Generic | ChartKind::Line) {
            self.kind = ChartKind::Area;
        }
        self
    }
    pub fn x_labels<T: 'static>(mut self, _value: T) -> Self {
        let value: Box<dyn Any> = Box::new(_value);
        let value = match value.downcast::<Vec<String>>() {
            Ok(labels) => {
                self.x_labels = *labels;
                None
            }
            Err(value) => Some(value),
        };
        if let Some(value) = value {
            if let Ok(labels) = value.downcast::<Vec<&'static str>>() {
                self.x_labels = labels.iter().map(|label| (*label).to_owned()).collect();
            }
        }
        self.kind = ChartKind::Heatmap;
        self
    }
    pub fn y_labels<T: 'static>(mut self, _value: T) -> Self {
        let value: Box<dyn Any> = Box::new(_value);
        let value = match value.downcast::<Vec<String>>() {
            Ok(labels) => {
                self.y_labels = *labels;
                None
            }
            Err(value) => Some(value),
        };
        if let Some(value) = value {
            if let Ok(labels) = value.downcast::<Vec<&'static str>>() {
                self.y_labels = labels.iter().map(|label| (*label).to_owned()).collect();
            }
        }
        self.kind = ChartKind::Heatmap;
        self
    }
    pub fn color_range(mut self, min: Color, max: Color) -> Self {
        self.color_min = min;
        self.color_max = max;
        self
    }
    pub fn color_stops<T: 'static>(mut self, value: T) -> Self {
        let value: Box<dyn Any> = Box::new(value);
        let value = match value.downcast::<Vec<(f32, Color)>>() {
            Ok(stops) => {
                self.color_stops = stops
                    .into_iter()
                    .filter(|(position, _)| position.is_finite())
                    .map(|(position, color)| (position.clamp(0.0, 1.0), color))
                    .collect();
                self.color_stops
                    .sort_by(|left, right| left.0.total_cmp(&right.0));
                None
            }
            Err(value) => Some(value),
        };
        if let Some(value) = value {
            if let Ok(colors) = value.downcast::<Vec<Color>>() {
                let last = colors.len().saturating_sub(1).max(1) as f32;
                self.color_stops = colors
                    .into_iter()
                    .enumerate()
                    .map(|(index, color)| (index as f32 / last, color))
                    .collect();
            }
        }
        self
    }
    pub fn calendar_mode(mut self, value: bool) -> Self {
        self.calendar_mode = value;
        self.kind = ChartKind::Heatmap;
        self
    }
    pub fn year(mut self, value: i32) -> Self {
        self.year = value;
        self
    }
    pub fn cell_size(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.cell_size = value.max(1.0);
        }
        self
    }
    pub fn cell_gap(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.cell_gap = value.max(0.0);
        }
        self
    }
    pub fn show_values(mut self, value: bool) -> Self {
        self.show_values = value;
        self
    }
    pub fn show_conversion_rate(mut self, value: bool) -> Self {
        self.show_conversion_rate = value;
        self
    }
    pub fn align(mut self, value: FunnelAlign) -> Self {
        self.funnel_align = value;
        self
    }
    pub fn gap(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.funnel_gap = value.max(0.0);
            self.treemap_gap = value.max(0.0);
        }
        self
    }
    pub fn bar_series<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(series) = any.downcast::<Vec<ChartSeries<Vec<BarData>>>>() {
            self.bar_series = *series;
            self.kind = ChartKind::Combo;
        }
        self
    }
    pub fn line_series<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(series) = any.downcast::<Vec<ChartSeries<Vec<LineData>>>>() {
            self.line_series = *series;
            self.kind = ChartKind::Combo;
        }
        self
    }
    pub fn y_axis_left(mut self, value: impl Into<String>) -> Self {
        self.y_axis = value.into();
        self
    }
    pub fn y_axis_right(mut self, value: impl Into<String>) -> Self {
        self.y_axis_right = value.into();
        self
    }
    pub fn reference_line(
        mut self,
        value: f32,
        label: impl Into<String>,
        style: LineStyle,
    ) -> Self {
        if value.is_finite() {
            self.reference_lines.push((value, label.into(), style));
        }
        self
    }
    pub fn value(mut self, value: f32) -> Self {
        self.gauge_value = if value.is_finite() { value } else { 0.0 };
        self.kind = ChartKind::Gauge;
        self
    }
    pub fn min(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.gauge_min = value;
        }
        self
    }
    pub fn max(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.gauge_max = value;
        }
        self
    }
    pub fn range_colors<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(ranges) = any.downcast::<Vec<GaugeRange>>() {
            self.gauge_ranges = *ranges;
            self.kind = ChartKind::Gauge;
        }
        self
    }
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = value.into();
        self
    }
    pub fn subtitle(mut self, value: impl Into<String>) -> Self {
        self.subtitle = value.into();
        self
    }
    pub fn gauge_type(mut self, value: GaugeType) -> Self {
        self.gauge_type = value;
        self.kind = ChartKind::Gauge;
        self
    }
    pub fn pointer_width(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.pointer_width = value.max(0.0);
        }
        self
    }
    pub fn pointer_color(mut self, value: Color) -> Self {
        self.pointer_color = Some(value);
        self
    }
    pub fn format<F>(mut self, value: F) -> Self
    where
        F: Fn(f32) -> String + 'static,
    {
        self.value_format = Some(Rc::new(value));
        self
    }
    pub fn bg(mut self, value: Color) -> Self {
        self.background = Some(value);
        self
    }
    pub fn padding(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.padding = value.max(0.0);
        }
        self
    }
    pub fn animation(mut self, value: AnimationConfig) -> Self {
        self.animation_config = Some(value);
        self.animation_player = Some(TransitionPlayer::new(value));
        self
    }
    pub fn responsive(mut self, value: bool) -> Self {
        self.responsive = value;
        self
    }
    pub fn interactive(mut self, value: InteractionConfig) -> Self {
        self.interaction = Some(value);
        self
    }
    pub fn brush(mut self, value: BrushConfig) -> Self {
        self.brush_config = Some(value);
        self
    }
    pub fn tooltip(mut self, value: TooltipConfig) -> Self {
        self.tooltip_config = Some(value);
        self
    }


}

