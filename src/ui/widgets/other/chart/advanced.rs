use std::any::Any;
use std::cell::Cell;
use std::fmt;
use std::rc::Rc;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, FillRule, PathBuilder};
use crate::ui::animation::{AnimationConfig, TransitionPlayer};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SystemEvent, WidgetTree};

use super::bar_chart::BarData;
use super::line_chart::LineData;

/// 多系列图表的通用数据容器。
#[derive(Debug, Clone, PartialEq)]
pub struct ChartSeries<T> {
    pub name: String,
    pub data: T,
}

impl<T> ChartSeries<T> {
    pub fn new(name: impl Into<String>, data: T) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LegendPosition {
    Top,
    Bottom,
    Left,
    Right,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoseStyle {
    Radius,
    Area,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelPosition {
    Inside,
    Outside,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointStyle {
    Circle,
    Diamond,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RadarShape {
    Polygon,
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunnelShape {
    Normal,
    Symmetric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FunnelAlign {
    Center,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChartType {
    Bar,
    Line,
    Area,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisSide {
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStyle {
    Solid,
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GaugeType {
    Dashboard,
    Full,
    Ring,
}

#[derive(Debug, Clone, Copy)]
pub struct InteractionConfig {
    pub zoom: bool,
    pub pan: bool,
    pub crosshair: bool,
    pub on_click: Option<fn(&str)>,
}

#[derive(Debug, Clone, Copy)]
pub struct BrushConfig {
    pub enabled: bool,
    pub on_select: Option<fn((f32, f32))>,
}

/// 图表 tooltip 的触发方式。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TooltipTrigger {
    /// 指针悬停时跟随当前数据项。
    #[default]
    Hover,
    /// 单击后固定到当前数据项，直至再次单击图表。
    Click,
}

/// 传给 tooltip 自定义渲染器的标准化数据。
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TooltipDatum {
    pub label: String,
    pub series: Option<String>,
    pub value: Option<f32>,
    pub x: Option<f32>,
    pub y: Option<f32>,
    pub percentage: Option<f32>,
}

type TooltipRenderer = Rc<dyn Fn(&TooltipDatum) -> String>;

/// 图表 tooltip 配置。
///
/// 模板支持 `{label}`、`{series}`、`{value}`、`{x}`、`{y}` 和
/// `{percentage}`；`render` 的优先级高于模板。
#[derive(Clone, Default)]
pub struct TooltipConfig {
    trigger: TooltipTrigger,
    template: Option<String>,
    renderer: Option<TooltipRenderer>,
}

impl fmt::Debug for TooltipConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TooltipConfig")
            .field("trigger", &self.trigger)
            .field("template", &self.template)
            .field("has_renderer", &self.renderer.is_some())
            .finish()
    }
}

impl TooltipConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn trigger(mut self, trigger: TooltipTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    pub fn template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    pub fn render<F>(mut self, renderer: F) -> Self
    where
        F: Fn(&TooltipDatum) -> String + 'static,
    {
        self.renderer = Some(Rc::new(renderer));
        self
    }

    pub(super) fn trigger_mode(&self) -> TooltipTrigger {
        self.trigger
    }

    pub(super) fn format(&self, datum: &TooltipDatum) -> String {
        if let Some(renderer) = &self.renderer {
            return renderer(datum);
        }
        if let Some(template) = &self.template {
            return template
                .replace("{label}", &datum.label)
                .replace("{series}", datum.series.as_deref().unwrap_or(""))
                .replace("{value}", &format_optional_number(datum.value))
                .replace("{x}", &format_optional_number(datum.x))
                .replace("{y}", &format_optional_number(datum.y))
                .replace(
                    "{percentage}",
                    &datum
                        .percentage
                        .map(|value| format!("{:.1}%", value * 100.0))
                        .unwrap_or_default(),
                );
        }
        datum.default_text()
    }
}

impl TooltipDatum {
    fn default_text(&self) -> String {
        let prefix = match (&self.series, self.label.is_empty()) {
            (Some(series), false) if !series.is_empty() => format!("{series} · {}", self.label),
            (Some(_), false) => self.label.clone(),
            (Some(series), true) => series.clone(),
            (None, false) => self.label.clone(),
            (None, true) => String::new(),
        };
        let mut detail = match (self.value, self.x, self.y) {
            (Some(value), _, _) => format_number(value),
            (None, Some(x), Some(y)) => format!("({}, {})", format_number(x), format_number(y)),
            (None, Some(x), None) => format_number(x),
            (None, None, Some(y)) => format_number(y),
            (None, None, None) => String::new(),
        };
        if let Some(percentage) = self.percentage.filter(|value| value.is_finite()) {
            if !detail.is_empty() {
                detail.push(' ');
            }
            detail.push_str(&format!("({:.1}%)", percentage * 100.0));
        }
        match (prefix.is_empty(), detail.is_empty()) {
            (false, false) => format!("{prefix}: {detail}"),
            (false, true) => prefix,
            (true, false) => detail,
            (true, true) => String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ScatterData {
    pub label: String,
    pub x: f32,
    pub y: f32,
}

impl ScatterData {
    pub fn new(label: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            label: label.into(),
            x,
            y,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BubbleData {
    pub label: String,
    pub x: f32,
    pub y: f32,
    pub size: f32,
}

impl BubbleData {
    pub fn new(label: impl Into<String>, x: f32, y: f32, size: f32) -> Self {
        Self {
            label: label.into(),
            x,
            y,
            size,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RadarAxis {
    pub label: String,
    pub range: std::ops::RangeInclusive<f32>,
}

impl RadarAxis {
    pub fn new(label: impl Into<String>, range: std::ops::RangeInclusive<f32>) -> Self {
        Self {
            label: label.into(),
            range,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RadarData {
    pub value: f32,
}

impl RadarData {
    pub fn new(value: f32) -> Self {
        Self { value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeatmapCell {
    pub x: usize,
    pub y: usize,
    pub value: f32,
}

impl HeatmapCell {
    pub fn new(x: usize, y: usize, value: f32) -> Self {
        Self { x, y, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FunnelData {
    pub label: String,
    pub value: f32,
}

impl FunnelData {
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterfallKind {
    Total,
    Increase,
    Decrease,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaterfallData {
    pub label: String,
    pub value: f32,
    pub kind: WaterfallKind,
}

impl WaterfallData {
    pub fn new(label: impl Into<String>, value: f32, kind: WaterfallKind) -> Self {
        Self {
            label: label.into(),
            value,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ComboSeries<T> {
    pub name: String,
    pub data: T,
    pub chart_type: ChartType,
    pub axis: AxisSide,
    pub line_style: LineStyle,
}

impl<T> ComboSeries<T> {
    pub fn new(name: impl Into<String>, data: T) -> Self {
        Self {
            name: name.into(),
            data,
            chart_type: ChartType::Line,
            axis: AxisSide::Left,
            line_style: LineStyle::Solid,
        }
    }

    pub fn chart_type(mut self, chart_type: ChartType) -> Self {
        self.chart_type = chart_type;
        self
    }

    pub fn y_axis(mut self, axis: AxisSide) -> Self {
        self.axis = axis;
        self
    }

    pub fn line_style(mut self, line_style: LineStyle) -> Self {
        self.line_style = line_style;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TreemapNode {
    pub label: String,
    pub value: f32,
    pub children: Vec<Self>,
}

impl TreemapNode {
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
            children: Vec::new(),
        }
    }

    pub fn children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GaugeRange {
    pub start: f32,
    pub end: f32,
    pub color: Color,
}

impl GaugeRange {
    pub fn new(start: f32, end: f32, color: Color) -> Self {
        Self { start, end, color }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ChartKind {
    Generic,
    Bar,
    Line,
    Area,
    Scatter,
    Radar,
    Heatmap,
    Funnel,
    Waterfall,
    Combo,
    Treemap,
    Gauge,
}

const MAX_HEATMAP_DIMENSION: usize = 4096;

#[derive(Clone)]
enum ChartPayload {
    Empty,
    Bars(Vec<BarData>),
    BarSeries(Vec<ChartSeries<Vec<BarData>>>),
    Lines(Vec<LineData>),
    LineSeries(Vec<ChartSeries<Vec<LineData>>>),
    Scatter(Vec<ScatterData>),
    Bubble(Vec<BubbleData>),
    ScatterSeries(Vec<ChartSeries<Vec<ScatterData>>>),
    Heatmap(Vec<HeatmapCell>),
    Funnel(Vec<FunnelData>),
    Waterfall(Vec<WaterfallData>),
    Treemap(Vec<TreemapNode>),
}

component! {
    pub struct ChartPlaceholder {
        width: f32,
        height: f32,
        kind: ChartKind,
        payload: ChartPayload,
        background: Option<Color>,
        padding: f32,
        legend: LegendPosition,
        title: String,
        subtitle: String,
        responsive: bool,
        grouped: bool,
        stacked: bool,
        horizontal: bool,
        smooth: bool,
        step: bool,
        bar_gap: f32,
        category_gap: f32,
        rose: bool,
        rose_style: RoseStyle,
        start_angle: f32,
        end_angle: f32,
        total: Option<f32>,
        label_visible: bool,
        label_position: LabelPosition,
        x_axis: String,
        y_axis: String,
        y_axis_right: String,
        bubble_scale: f32,
        point_size: f32,
        point_style: PointStyle,
        grid_levels: usize,
        fill_opacity: f32,
        color_min: Color,
        color_max: Color,
        calendar_mode: bool,
        year: i32,
        cell_size: f32,
        cell_gap: f32,
        show_values: bool,
        show_conversion_rate: bool,
        funnel_align: FunnelAlign,
        funnel_shape: FunnelShape,
        funnel_gap: f32,
        treemap_gap: f32,
        bar_series: Vec<ChartSeries<Vec<BarData>>>,
        line_series: Vec<ChartSeries<Vec<LineData>>>,
        combo_series: Vec<ComboSeries<Vec<LineData>>>,
        scatter_series: Vec<ChartSeries<Vec<ScatterData>>>,
        radar_axes: Vec<RadarAxis>,
        radar_series: Vec<ChartSeries<Vec<RadarData>>>,
        radar_shape: RadarShape,
        gauge_ranges: Vec<GaugeRange>,
        gauge_value: f32,
        gauge_min: f32,
        gauge_max: f32,
        gauge_type: GaugeType,
        pointer_width: f32,
        pointer_color: Option<Color>,
        value_format: Option<Rc<dyn Fn(f32) -> String>>,
        interaction: Option<InteractionConfig>,
        brush_config: Option<BrushConfig>,
        tooltip_config: Option<TooltipConfig>,
        x_labels: Vec<String>,
        y_labels: Vec<String>,
        color_stops: Vec<(f32, Color)>,
        reference_lines: Vec<(f32, String, LineStyle)>,
        animation_config: Option<AnimationConfig>,
        #[snapshot(skip)]
        animation_player: Option<TransitionPlayer>,
        #[snapshot(skip)]
        animation_dirty: Cell<bool>,
        #[snapshot(skip)]
        last_frame: Cell<Option<Rect>>,
        #[snapshot(skip)]
        hovered_pos: Cell<Option<Point>>,
        #[snapshot(skip)]
        tooltip_pos: Cell<Option<Point>>,
        #[snapshot(skip)]
        brush_start: Cell<Option<Point>>,
        #[snapshot(skip)]
        pan_start: Cell<Option<Point>>,
        #[snapshot(skip)]
        pan_origin: Cell<f32>,
        #[snapshot(skip)]
        pan_offset: Cell<f32>,
        #[snapshot(skip)]
        zoom: Cell<f32>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let width = super::responsive_extent(self.responsive, constraints.max.w, self.width);
        let height = super::responsive_extent(self.responsive, constraints.max.h, self.height);
        constraints.clamp(Size::new(width, height))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        let Some(frame) = self.last_frame.get() else {
            return EventResult::NotHandled;
        };
        match event {
            SystemEvent::PointerMove { pos, .. } => {
                let inside = frame.contains(*pos);
                if inside {
                    self.hovered_pos.set(Some(*pos));
                } else {
                    self.hovered_pos.set(None);
                }
                if let Some(start) = self.pan_start.get() {
                    let offset = (self.pan_origin.get() + pos.x - start.x).clamp(-frame.w, frame.w);
                    self.pan_offset.set(offset);
                    return EventResult::Handled;
                }
                let hover_tooltip = self
                    .tooltip_config
                    .as_ref()
                    .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Hover);
                if inside
                    && (self.interaction.is_some()
                        || self.brush_config.is_some()
                        || hover_tooltip)
                {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered_pos.take().is_some();
                if changed { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerDown { pos, button: MouseButton::Left, .. }
                if frame.contains(*pos) =>
            {
                let mut handled = false;
                if self.interaction.as_ref().is_some_and(|config| config.pan) {
                    self.pan_start.set(Some(*pos));
                    self.pan_origin.set(self.pan_offset.get());
                    handled = true;
                }
                if self.brush_config.as_ref().is_some_and(|config| config.enabled) {
                    self.brush_start.set(Some(*pos));
                    handled = true;
                }
                if let Some(config) = self.interaction.as_ref() {
                    if let Some(callback) = config.on_click {
                        let label = self.data_label_at(*pos, frame);
                        callback(&label);
                        handled = true;
                    }
                }
                if self
                    .tooltip_config
                    .as_ref()
                    .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Click)
                {
                    self.tooltip_pos
                        .set(self.tooltip_pos.get().is_none().then_some(*pos));
                    handled = true;
                }
                if handled { EventResult::Handled } else { EventResult::NotHandled }
            }
            SystemEvent::PointerUp { pos, button: MouseButton::Left, .. } => {
                let was_panning = self.pan_start.take().is_some();
                let Some(start) = self.brush_start.take() else {
                    return if was_panning {
                        EventResult::Handled
                    } else {
                        EventResult::NotHandled
                    };
                };
                if let Some(config) = self.brush_config.as_ref().filter(|config| config.enabled) {
                    if let Some(callback) = config.on_select {
                        let start_x = ((start.x - frame.x) / frame.w.max(f32::EPSILON)).clamp(0.0, 1.0);
                        let end_x = ((pos.x - frame.x) / frame.w.max(f32::EPSILON)).clamp(0.0, 1.0);
                        callback((start_x.min(end_x), start_x.max(end_x)));
                    }
                }
                EventResult::Handled
            }
            SystemEvent::Wheel { pos, delta } if frame.contains(*pos) => {
                if self.interaction.as_ref().is_some_and(|config| config.zoom) {
                    let factor = if delta.y.is_finite() {
                        (1.0 - delta.y * 0.001).max(0.01)
                    } else {
                        1.0
                    };
                    let next = (self.zoom.get() * factor).clamp(1.0, 8.0);
                    self.zoom.set(next);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    wants_continuous_pointer_move => (&self) -> bool {
        self.interaction.as_ref().is_some_and(|config| config.crosshair || config.pan)
            || self.brush_config.as_ref().is_some_and(|config| config.enabled)
            || self
                .tooltip_config
                .as_ref()
                .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Hover)
    }

    update_animation => (&mut self, dt: f64) -> bool {
        let Some(player) = self.animation_player.as_mut() else {
            self.animation_dirty.set(false);
            return false;
        };
        if player.finished {
            self.animation_dirty.set(false);
            return false;
        }
        player.update(if dt.is_finite() { dt.max(0.0) } else { 0.0 });
        self.animation_dirty.set(true);
        !player.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.animation_dirty.replace(false) { frame } else { Rect::zero() }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.paint(ctx, frame);
    }
}

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

    fn new_with_kind(kind: ChartKind) -> Self {
        let mut chart = Self::new();
        chart.kind = kind;
        chart
    }

    pub fn data<T: 'static>(mut self, data: T) -> Self {
        let mut any: Box<dyn Any> = Box::new(data);
        macro_rules! take_payload {
            ($ty:ty, $payload:expr, $kind:expr) => {
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

    fn has_data(&self) -> bool {
        match self.kind {
            ChartKind::Generic => false,
            ChartKind::Bar => match &self.payload {
                ChartPayload::Bars(data) => !data.is_empty(),
                ChartPayload::BarSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self.bar_series.iter().any(|series| !series.data.is_empty()),
            },
            ChartKind::Line | ChartKind::Area => match &self.payload {
                ChartPayload::Lines(data) => !data.is_empty(),
                ChartPayload::LineSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self
                    .line_series
                    .iter()
                    .any(|series| !series.data.is_empty()),
            },
            ChartKind::Scatter => match &self.payload {
                ChartPayload::Scatter(data) => !data.is_empty(),
                ChartPayload::Bubble(data) => !data.is_empty(),
                ChartPayload::ScatterSeries(series) => {
                    series.iter().any(|series| !series.data.is_empty())
                }
                _ => self
                    .scatter_series
                    .iter()
                    .any(|series| !series.data.is_empty()),
            },
            ChartKind::Radar => self
                .radar_series
                .iter()
                .any(|series| !series.data.is_empty()),
            ChartKind::Heatmap => {
                matches!(&self.payload, ChartPayload::Heatmap(data) if data.iter().any(|cell| {
                    if self.calendar_mode {
                        cell.x < calendar_day_count(self.year)
                    } else {
                        cell.x < MAX_HEATMAP_DIMENSION && cell.y < MAX_HEATMAP_DIMENSION
                    }
                }))
            }
            ChartKind::Funnel => {
                matches!(&self.payload, ChartPayload::Funnel(data) if !data.is_empty())
            }
            ChartKind::Waterfall => {
                matches!(&self.payload, ChartPayload::Waterfall(data) if !data.is_empty())
            }
            ChartKind::Combo => {
                !self.combo_series.is_empty()
                    || self.bar_series.iter().any(|series| !series.data.is_empty())
                    || self
                        .line_series
                        .iter()
                        .any(|series| !series.data.is_empty())
            }
            ChartKind::Treemap => {
                matches!(&self.payload, ChartPayload::Treemap(data) if !data.is_empty())
            }
            ChartKind::Gauge => true,
        }
    }

    fn category_index(pos: Point, frame: Rect, count: usize) -> usize {
        if count == 0 || frame.w <= 0.0 {
            return 0;
        }
        (((pos.x - frame.x) / frame.w * count as f32).floor() as usize).min(count - 1)
    }

    fn data_label_at(&self, pos: Point, frame: Rect) -> String {
        self.tooltip_datum_at(pos, frame)
            .map(|datum| datum.label)
            .unwrap_or_default()
    }

    fn tooltip_datum_at(&self, pos: Point, frame: Rect) -> Option<TooltipDatum> {
        let index = |count| Self::category_index(pos, frame, count);
        let payload_datum = match &self.payload {
            ChartPayload::Bars(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::BarSeries(series) => series.first().and_then(|series| {
                series
                    .data
                    .get(index(series.data.len()))
                    .map(|item| TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    })
            }),
            ChartPayload::Lines(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::LineSeries(series) => series.first().and_then(|series| {
                series
                    .data
                    .get(index(series.data.len()))
                    .map(|item| TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    })
            }),
            ChartPayload::Scatter(data) => data.get(index(data.len())).map(scatter_tooltip_datum),
            ChartPayload::ScatterSeries(series) => series.first().and_then(|series| {
                series.data.get(index(series.data.len())).map(|item| {
                    let mut datum = scatter_tooltip_datum(item);
                    datum.series = Some(series.name.clone());
                    datum
                })
            }),
            ChartPayload::Bubble(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                x: Some(finite_or_zero(item.x)),
                y: Some(finite_or_zero(item.y)),
                ..TooltipDatum::default()
            }),
            ChartPayload::Heatmap(cells) => {
                let columns = if self.calendar_mode {
                    let offset = january_first_weekday(self.year);
                    (offset + calendar_day_count(self.year) - 1) / 7 + 1
                } else {
                    cells
                        .iter()
                        .map(|cell| cell.x)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(MAX_HEATMAP_DIMENSION)
                };
                let rows = if self.calendar_mode {
                    7
                } else {
                    cells
                        .iter()
                        .map(|cell| cell.y)
                        .max()
                        .unwrap_or(0)
                        .saturating_add(1)
                        .min(MAX_HEATMAP_DIMENSION)
                };
                let x = normalized_grid_index(pos.x, frame.x, frame.w, columns);
                let y = normalized_grid_index(pos.y, frame.y, frame.h, rows);
                cells
                    .iter()
                    .find(|cell| {
                        if self.calendar_mode {
                            let ordinal = january_first_weekday(self.year) + cell.x;
                            cell.x < calendar_day_count(self.year)
                                && ordinal / 7 == x
                                && ordinal % 7 == y
                        } else {
                            cell.x == x && cell.y == y
                        }
                    })
                    .map(|cell| {
                        let x_label = self
                            .x_labels
                            .get(cell.x)
                            .cloned()
                            .unwrap_or_else(|| cell.x.to_string());
                        let y_label = self
                            .y_labels
                            .get(cell.y)
                            .cloned()
                            .unwrap_or_else(|| cell.y.to_string());
                        TooltipDatum {
                            label: format!("{y_label} {x_label}"),
                            value: Some(finite_or_zero(cell.value)),
                            ..TooltipDatum::default()
                        }
                    })
            }
            ChartPayload::Funnel(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::Waterfall(data) => data.get(index(data.len())).map(|item| TooltipDatum {
                label: item.label.clone(),
                value: Some(finite_or_zero(item.value)),
                ..TooltipDatum::default()
            }),
            ChartPayload::Treemap(data) => {
                let total = data
                    .iter()
                    .map(|node| finite_or_zero(node.value).max(0.0))
                    .sum::<f32>();
                data.get(index(data.len())).map(|item| {
                    let value = finite_or_zero(item.value);
                    TooltipDatum {
                        label: item.label.clone(),
                        value: Some(value),
                        percentage: (total > 0.0).then_some(value.max(0.0) / total),
                        ..TooltipDatum::default()
                    }
                })
            }
            ChartPayload::Empty => None,
        };
        if payload_datum.is_some() {
            return payload_datum;
        }
        match self.kind {
            ChartKind::Radar => {
                let series = self.radar_series.first()?;
                let item_index = index(series.data.len());
                let value = series.data.get(item_index)?;
                Some(TooltipDatum {
                    label: self
                        .radar_axes
                        .get(item_index)
                        .map(|axis| axis.label.clone())
                        .unwrap_or_else(|| item_index.to_string()),
                    series: Some(series.name.clone()),
                    value: Some(finite_or_zero(value.value)),
                    ..TooltipDatum::default()
                })
            }
            ChartKind::Combo => {
                if let Some(series) = self.combo_series.first() {
                    let item = series.data.get(index(series.data.len()))?;
                    return Some(TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    });
                }
                if let Some(series) = self.bar_series.first() {
                    let item = series.data.get(index(series.data.len()))?;
                    return Some(TooltipDatum {
                        label: item.label.clone(),
                        series: Some(series.name.clone()),
                        value: Some(finite_or_zero(item.value)),
                        ..TooltipDatum::default()
                    });
                }
                let series = self.line_series.first()?;
                let item = series.data.get(index(series.data.len()))?;
                Some(TooltipDatum {
                    label: item.label.clone(),
                    series: Some(series.name.clone()),
                    value: Some(finite_or_zero(item.value)),
                    ..TooltipDatum::default()
                })
            }
            ChartKind::Gauge => Some(TooltipDatum {
                label: self.title.clone(),
                value: Some(finite_or_zero(self.gauge_value)),
                ..TooltipDatum::default()
            }),
            _ => None,
        }
    }

    fn tooltip_text_at(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        let datum = self.tooltip_datum_at(pos, frame)?;
        Some(config.format(&datum))
    }

    fn legend_labels(&self) -> Vec<String> {
        let series_names = |series: &[ChartSeries<Vec<LineData>>]| {
            series
                .iter()
                .map(|series| series.name.clone())
                .collect::<Vec<_>>()
        };
        match &self.payload {
            ChartPayload::BarSeries(series) => {
                series.iter().map(|series| series.name.clone()).collect()
            }
            ChartPayload::LineSeries(series) => series_names(series),
            ChartPayload::ScatterSeries(series) => {
                series.iter().map(|series| series.name.clone()).collect()
            }
            ChartPayload::Funnel(data) => {
                data.iter().take(8).map(|item| item.label.clone()).collect()
            }
            ChartPayload::Waterfall(data) => {
                data.iter().take(8).map(|item| item.label.clone()).collect()
            }
            ChartPayload::Treemap(nodes) => nodes
                .iter()
                .take(8)
                .map(|node| node.label.clone())
                .collect(),
            ChartPayload::Empty if !self.combo_series.is_empty() => self
                .combo_series
                .iter()
                .map(|series| series.name.clone())
                .collect(),
            ChartPayload::Empty if !self.radar_series.is_empty() => self
                .radar_series
                .iter()
                .map(|series| series.name.clone())
                .collect(),
            ChartPayload::Empty if !self.bar_series.is_empty() || !self.line_series.is_empty() => {
                self.bar_series
                    .iter()
                    .map(|series| series.name.clone())
                    .chain(self.line_series.iter().map(|series| series.name.clone()))
                    .collect()
            }
            ChartPayload::Empty if self.kind == ChartKind::Gauge => {
                vec![if self.title.is_empty() {
                    "值".to_owned()
                } else {
                    self.title.clone()
                }]
            }
            ChartPayload::Empty => Vec::new(),
            _ => vec!["数据".to_owned()],
        }
    }

    fn legend_layout(&self, plot: Rect) -> (Rect, Option<Rect>) {
        if self.legend == LegendPosition::None || self.legend_labels().is_empty() {
            return (plot, None);
        }
        const ROW: f32 = 18.0;
        match self.legend {
            LegendPosition::Top => (
                Rect::new(plot.x, plot.y + ROW, plot.w, (plot.h - ROW).max(0.0)),
                Some(Rect::new(plot.x, plot.y, plot.w, ROW)),
            ),
            LegendPosition::Bottom => (
                Rect::new(plot.x, plot.y, plot.w, (plot.h - ROW).max(0.0)),
                Some(Rect::new(
                    plot.x,
                    plot.y + (plot.h - ROW).max(0.0),
                    plot.w,
                    ROW.min(plot.h),
                )),
            ),
            LegendPosition::Left | LegendPosition::Right => {
                let width = (plot.w * 0.24).clamp(64.0, 120.0).min(plot.w);
                if self.legend == LegendPosition::Left {
                    (
                        Rect::new(plot.x + width, plot.y, (plot.w - width).max(0.0), plot.h),
                        Some(Rect::new(plot.x, plot.y, width, plot.h)),
                    )
                } else {
                    (
                        Rect::new(plot.x, plot.y, (plot.w - width).max(0.0), plot.h),
                        Some(Rect::new(
                            plot.x + (plot.w - width).max(0.0),
                            plot.y,
                            width,
                            plot.h,
                        )),
                    )
                }
            }
            LegendPosition::None => (plot, None),
        }
    }

    fn paint_legend(&self, ctx: &mut PaintContext, rect: Rect) {
        let labels = self.legend_labels();
        if labels.is_empty() || rect.w <= 0.0 || rect.h <= 0.0 {
            return;
        }
        let color = ctx.tokens().color_text_secondary();
        if matches!(self.legend, LegendPosition::Top | LegendPosition::Bottom) {
            ctx.text_center(&labels.join("  "), rect, color, 10.0);
            return;
        }
        for (index, label) in labels.iter().enumerate() {
            let y = rect.y + index as f32 * 18.0;
            if y + 18.0 > rect.y + rect.h {
                break;
            }
            ctx.fill_rect(
                Rect::new(rect.x + 4.0, y + 5.0, 8.0, 8.0),
                palette_color(index),
                None,
            );
            ctx.draw_text(label, Point::new(rect.x + 16.0, y + 3.0), color, 10.0);
        }
    }

    fn paint(&self, ctx: &mut PaintContext, frame: Rect) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        self.last_frame.set(Some(frame));
        let bg = self.background.unwrap_or(ctx.tokens().color_bg_container());
        ctx.fill_rect(frame, bg, None);
        let mut plot = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            ctx.draw_text(
                &self.title,
                Point::new(plot.x, plot.y),
                ctx.tokens().color_text(),
                15.0,
            );
            plot.y += 20.0;
            plot.h = (plot.h - 20.0).max(0.0);
        }
        if !self.subtitle.is_empty() && plot.h > 0.0 {
            ctx.draw_text(
                &self.subtitle,
                Point::new(plot.x, plot.y),
                ctx.tokens().color_text_secondary(),
                11.0,
            );
            plot.y += 16.0;
            plot.h = (plot.h - 16.0).max(0.0);
        }
        if plot.w <= 0.0 || plot.h <= 0.0 {
            return;
        }
        let (mut plot, legend_rect) = self.legend_layout(plot);
        if plot.w <= 0.0 || plot.h <= 0.0 {
            if let Some(legend_rect) = legend_rect {
                self.paint_legend(ctx, legend_rect);
            }
            return;
        }
        let base_width = plot.w;
        let zoom = self.zoom.get();
        if zoom > 1.0 {
            plot.w = base_width * zoom;
            plot.x += (base_width - plot.w) * 0.5;
        }
        plot.x += self.pan_offset.get();
        ctx.push_clip(frame);
        let progress = self
            .animation_player
            .as_ref()
            .map_or(1.0, |player| player.opacity_progress.clamp(0.0, 1.0));
        let animated = self.animation_config.is_some() && progress < 1.0;
        if animated {
            ctx.push_clip(Rect::new(plot.x, plot.y, plot.w * progress, plot.h));
        }
        if self.has_data() {
            match self.kind {
                ChartKind::Bar => self.paint_bars(ctx, plot),
                ChartKind::Line | ChartKind::Area => self.paint_lines(ctx, plot),
                ChartKind::Scatter => self.paint_scatter(ctx, plot),
                ChartKind::Radar => self.paint_radar(ctx, plot),
                ChartKind::Heatmap => self.paint_heatmap(ctx, plot),
                ChartKind::Funnel => self.paint_funnel(ctx, plot),
                ChartKind::Waterfall => self.paint_waterfall(ctx, plot),
                ChartKind::Combo => self.paint_combo(ctx, plot),
                ChartKind::Treemap => self.paint_treemap(ctx, plot),
                ChartKind::Gauge => self.paint_gauge(ctx, plot),
                ChartKind::Generic => unreachable!("generic charts do not contain data"),
            }
        } else {
            ctx.stroke_rect(plot, ctx.tokens().color_border(), 1.0, None);
            ctx.text_center("暂无数据", plot, ctx.tokens().color_text_secondary(), 12.0);
        }
        if animated {
            ctx.pop_clip();
        }
        if let Some(legend_rect) = legend_rect {
            self.paint_legend(ctx, legend_rect);
        }
        self.paint_interaction(ctx, frame);
        ctx.pop_clip();
    }

    fn paint_interaction(&self, ctx: &mut PaintContext, frame: Rect) {
        if let Some(start) = self.brush_start.get() {
            if let Some(end) = self.hovered_pos.get() {
                let x = start.x.min(end.x).clamp(frame.x, frame.x + frame.w);
                let y = start.y.min(end.y).clamp(frame.y, frame.y + frame.h);
                let right = start.x.max(end.x).clamp(frame.x, frame.x + frame.w);
                let bottom = start.y.max(end.y).clamp(frame.y, frame.y + frame.h);
                ctx.fill_rect(
                    Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0)),
                    Color::from_rgba(22, 119, 255, 48),
                    None,
                );
            }
        }
        let hovered = self.hovered_pos.get().filter(|pos| frame.contains(*pos));
        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = hovered {
                let color = ctx.tokens().color_primary();
                ctx.draw_line(pos.x, frame.y, pos.x, frame.y + frame.h, color, 1.0);
                ctx.draw_line(frame.x, pos.y, frame.x + frame.w, pos.y, color, 1.0);
            }
        }
        if let Some(config) = &self.tooltip_config {
            let pos = match config.trigger_mode() {
                TooltipTrigger::Hover => hovered,
                TooltipTrigger::Click => self.tooltip_pos.get().filter(|pos| frame.contains(*pos)),
            };
            let Some(pos) = pos else { return };
            if let Some(text) = self.tooltip_text_at(pos, frame) {
                let size = ctx.measure_text(&text, 10.0);
                let x = (pos.x + 12.0).min(frame.x + frame.w - size.w - 12.0);
                let y = (pos.y - size.h - 12.0).max(frame.y + 4.0);
                let rect = Rect::new(x.max(frame.x + 4.0), y, size.w + 8.0, size.h + 8.0);
                ctx.fill_rect(rect, ctx.tokens().color_bg_elevated(), None);
                ctx.stroke_rect(rect, ctx.tokens().color_border(), 1.0, None);
                ctx.draw_text(
                    &text,
                    Point::new(rect.x + 4.0, rect.y + 4.0),
                    ctx.tokens().color_text(),
                    10.0,
                );
            }
        }
    }

    fn paint_axes(&self, ctx: &mut PaintContext, plot: Rect) {
        let axis = ctx.tokens().color_border();
        ctx.fill_rect(
            Rect::new(plot.x, plot.y + plot.h - 1.0, plot.w, 1.0),
            axis,
            None,
        );
        ctx.fill_rect(Rect::new(plot.x, plot.y, 1.0, plot.h), axis, None);
        if !self.x_axis.is_empty() {
            ctx.text_center(
                &self.x_axis,
                Rect::new(plot.x, plot.y + plot.h, plot.w, 16.0),
                ctx.tokens().color_text_secondary(),
                10.0,
            );
        }
        if !self.y_axis.is_empty() {
            ctx.draw_text(
                &self.y_axis,
                Point::new(plot.x, plot.y - 2.0),
                ctx.tokens().color_text_secondary(),
                10.0,
            );
        }
        let max = self
            .reference_lines
            .iter()
            .map(|(value, _, _)| value.abs())
            .fold(1.0_f32, f32::max);
        for (value, label, style) in &self.reference_lines {
            let y = plot.y + plot.h - plot.h * (value / max).clamp(-1.0, 1.0).abs();
            let color = ctx.tokens().color_warning();
            let segments = if *style == LineStyle::Dashed { 12 } else { 1 };
            for segment in 0..segments {
                if *style == LineStyle::Dashed && segment % 2 == 1 {
                    continue;
                }
                let start = plot.x + plot.w * segment as f32 / segments as f32;
                let end = plot.x + plot.w * (segment + 1) as f32 / segments as f32;
                ctx.draw_line(start, y, end, y, color, 1.0);
            }
            if !label.is_empty() {
                ctx.draw_text(label, Point::new(plot.x + 4.0, y - 2.0), color, 9.0);
            }
        }
    }

    fn paint_bars(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        let series: Vec<&[BarData]> = match &self.payload {
            ChartPayload::Bars(data) => vec![data.as_slice()],
            ChartPayload::BarSeries(series) => {
                series.iter().map(|series| series.data.as_slice()).collect()
            }
            ChartPayload::Empty if !self.bar_series.is_empty() => self
                .bar_series
                .iter()
                .map(|series| series.data.as_slice())
                .collect(),
            _ => Vec::new(),
        };
        let count = series.iter().map(|items| items.len()).max().unwrap_or(0);
        if count == 0 {
            return;
        }
        let mut min_value: f32 = 0.0;
        let mut max_value: f32 = 0.0;
        for items in &series {
            for item in *items {
                let value = if item.value.is_finite() {
                    item.value
                } else {
                    0.0
                };
                min_value = min_value.min(value);
                max_value = max_value.max(value);
            }
        }
        let value_to_y =
            |value: f32| plot.y + plot.h - normalized_ratio(value, min_value, max_value) * plot.h;
        let value_to_x =
            |value: f32| plot.x + normalized_ratio(value, min_value, max_value) * plot.w;
        let category_w = plot.w / count as f32;
        let category_h = plot.h / count as f32;
        let grouped = self.grouped || (!self.stacked && series.len() > 1);
        let group_count = if grouped { series.len() } else { 1 } as f32;
        let mut positive_stack = vec![0.0_f32; count];
        let mut negative_stack = vec![0.0_f32; count];
        for (series_index, items) in series.iter().enumerate() {
            for index in 0..count {
                let Some(item) = items.get(index) else {
                    continue;
                };
                let value = if item.value.is_finite() {
                    item.value
                } else {
                    0.0
                };
                let (start, end) = if self.stacked {
                    if value >= 0.0 {
                        let start = positive_stack[index];
                        positive_stack[index] += value;
                        (start, positive_stack[index])
                    } else {
                        let start = negative_stack[index];
                        negative_stack[index] += value;
                        (start, negative_stack[index])
                    }
                } else {
                    (0.0, value)
                };
                if self.horizontal {
                    let h = (category_h * (1.0 - self.category_gap)).max(1.0);
                    let y = plot.y
                        + index as f32 * category_h
                        + category_h * self.category_gap * 0.5
                        + if grouped {
                            series_index as f32 * h / group_count
                        } else {
                            0.0
                        };
                    let bar_h = if grouped {
                        (h / group_count) * (1.0 - self.bar_gap)
                    } else {
                        h
                    };
                    let x = value_to_x(start.min(end));
                    let right = value_to_x(start.max(end));
                    ctx.fill_rect(
                        Rect::new(x, y, (right - x).max(0.0), bar_h),
                        item.color,
                        None,
                    );
                    if series_index == 0 {
                        ctx.draw_text(
                            &item.label,
                            Point::new(plot.x + plot.w + 4.0, y + bar_h * 0.5),
                            ctx.tokens().color_text_secondary(),
                            10.0,
                        );
                    }
                } else {
                    let w = (category_w * (1.0 - self.category_gap)).max(1.0);
                    let x = plot.x
                        + index as f32 * category_w
                        + category_w * self.category_gap * 0.5
                        + if grouped {
                            series_index as f32 * w / group_count
                        } else {
                            0.0
                        };
                    let bar_w = if grouped {
                        (w / group_count) * (1.0 - self.bar_gap)
                    } else {
                        w
                    };
                    let y = value_to_y(start.max(end));
                    let bottom = value_to_y(start.min(end));
                    ctx.fill_rect(
                        Rect::new(x, y, bar_w, (bottom - y).max(0.0)),
                        item.color,
                        None,
                    );
                    if series_index == 0 {
                        ctx.text_center(
                            &item.label,
                            Rect::new(x, plot.y + plot.h + 2.0, bar_w, 14.0),
                            ctx.tokens().color_text_secondary(),
                            10.0,
                        );
                    }
                }
            }
        }
    }

    fn paint_lines(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        let series: Vec<Vec<LineData>> = match &self.payload {
            ChartPayload::Lines(data) => vec![data.clone()],
            ChartPayload::LineSeries(series) => {
                series.iter().map(|series| series.data.clone()).collect()
            }
            ChartPayload::Empty if !self.line_series.is_empty() => self
                .line_series
                .iter()
                .map(|series| series.data.clone())
                .collect(),
            _ => Vec::new(),
        };
        let count = series.iter().map(Vec::len).max().unwrap_or(0);
        if count == 0 {
            return;
        }
        let mut lower_values = vec![vec![0.0_f32; count]; series.len()];
        let mut upper_values = vec![vec![0.0_f32; count]; series.len()];
        let mut min_value = 0.0_f32;
        let mut max_value = 0.0_f32;
        for (series_index, data) in series.iter().enumerate() {
            for index in 0..count {
                let raw = data.get(index).map_or(0.0, |item| item.value);
                let value = if raw.is_finite() { raw } else { 0.0 };
                let lower = if self.stacked && series_index > 0 {
                    upper_values[series_index - 1][index]
                } else {
                    0.0
                };
                let upper = if self.stacked { lower + value } else { value };
                lower_values[series_index][index] = if self.stacked { lower } else { 0.0 };
                upper_values[series_index][index] = upper;
                min_value = min_value.min(lower.min(upper));
                max_value = max_value.max(lower.max(upper));
            }
        }
        let to_point = |index: usize, value: f32| {
            let x = if count == 1 {
                plot.x + plot.w * 0.5
            } else {
                plot.x + index as f32 * plot.w / (count - 1) as f32
            };
            Point::new(
                x,
                plot.y + plot.h - normalized_ratio(value, min_value, max_value) * plot.h,
            )
        };
        for (series_index, data) in series.iter().enumerate() {
            let points = (0..data.len().min(count))
                .map(|index| to_point(index, upper_values[series_index][index]))
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let color = palette_color(series_index);
            if self.kind == ChartKind::Area && points.len() >= 2 {
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, points[0].y);
                for point in points.iter().skip(1) {
                    path.line_to(point.x, point.y);
                }
                for index in (0..points.len()).rev() {
                    let lower = to_point(index, lower_values[series_index][index]);
                    path.line_to(lower.x, lower.y);
                }
                path.close();
                let fill =
                    Color::from_rgba(color.r, color.g, color.b, (255.0 * self.fill_opacity) as u8);
                ctx.fill_path(&path.build(), fill, FillRule::NonZero);
            }
            if self.step {
                for pair in points.windows(2) {
                    ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[0].y, color, 2.0);
                    ctx.draw_line(pair[1].x, pair[0].y, pair[1].x, pair[1].y, color, 2.0);
                }
            } else {
                let line_points = if self.smooth {
                    catmull_rom_points(&points, 8)
                } else {
                    points.clone()
                };
                for pair in line_points.windows(2) {
                    ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, color, 2.0);
                }
            }
            for point in points {
                ctx.fill_circle(point.x, point.y, self.point_size, color);
            }
        }
    }

    fn paint_scatter(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        let series: Vec<Vec<(f32, f32, f32, bool)>> = match &self.payload {
            ChartPayload::Scatter(data) => vec![data
                .iter()
                .map(|item| (item.x, item.y, self.point_size, false))
                .collect()],
            ChartPayload::Bubble(data) => vec![data
                .iter()
                .map(|item| (item.x, item.y, item.size * self.bubble_scale, true))
                .collect()],
            ChartPayload::ScatterSeries(series) => series
                .iter()
                .map(|series| {
                    series
                        .data
                        .iter()
                        .map(|item| (item.x, item.y, self.point_size, false))
                        .collect()
                })
                .collect(),
            _ => Vec::new(),
        };
        let points = series
            .iter()
            .flatten()
            .filter(|point| point.0.is_finite() && point.1.is_finite())
            .collect::<Vec<_>>();
        if points.is_empty() {
            return;
        }
        let min_x = points
            .iter()
            .map(|point| point.0)
            .fold(f32::INFINITY, f32::min);
        let max_x = points
            .iter()
            .map(|point| point.0)
            .fold(f32::NEG_INFINITY, f32::max);
        let min_y = points
            .iter()
            .map(|point| point.1)
            .fold(f32::INFINITY, f32::min);
        let max_y = points
            .iter()
            .map(|point| point.1)
            .fold(f32::NEG_INFINITY, f32::max);
        let dx = (max_x - min_x).max(1.0);
        let dy = (max_y - min_y).max(1.0);
        for (series_index, data) in series.iter().enumerate() {
            let point_color = palette_color(series_index);
            for (x, y, radius, bubbles) in data {
                if !x.is_finite() || !y.is_finite() {
                    continue;
                }
                let px = plot.x + (x - min_x) / dx * plot.w;
                let py = plot.y + plot.h - (y - min_y) / dy * plot.h;
                let radius = if *bubbles {
                    finite_or_zero(*radius).clamp(2.0, 24.0)
                } else {
                    finite_or_zero(*radius).clamp(1.0, 12.0)
                };
                match self.point_style {
                    PointStyle::Circle => ctx.fill_circle(px, py, radius, point_color),
                    PointStyle::Diamond => {
                        ctx.fill_rect(
                            Rect::new(px - radius, py - radius, radius * 2.0, radius * 2.0),
                            point_color,
                            None,
                        );
                    }
                    PointStyle::Cross => {
                        ctx.draw_line(px - radius, py, px + radius, py, point_color, 2.0);
                        ctx.draw_line(px, py - radius, px, py + radius, point_color, 2.0);
                    }
                }
            }
        }
    }

    fn paint_radar(&self, ctx: &mut PaintContext, plot: Rect) {
        let count = self
            .radar_axes
            .len()
            .max(
                self.radar_series
                    .iter()
                    .map(|series| series.data.len())
                    .max()
                    .unwrap_or(0),
            )
            .max(3);
        let center = Point::new(plot.x + plot.w * 0.5, plot.y + plot.h * 0.5);
        let radius = plot.w.min(plot.h) * 0.38;
        let grid = ctx.tokens().color_border();
        for level in 1..=self.grid_levels.max(1) {
            let r = radius * level as f32 / self.grid_levels.max(1) as f32;
            if self.radar_shape == RadarShape::Circle {
                ctx.stroke_circle(center.x, center.y, r, grid, 1.0);
            } else {
                for axis in 0..count {
                    let a = -std::f32::consts::FRAC_PI_2
                        + axis as f32 * std::f32::consts::TAU / count as f32;
                    let next = -std::f32::consts::FRAC_PI_2
                        + (axis + 1) as f32 * std::f32::consts::TAU / count as f32;
                    ctx.draw_line(
                        center.x + r * a.cos(),
                        center.y + r * a.sin(),
                        center.x + r * next.cos(),
                        center.y + r * next.sin(),
                        grid,
                        1.0,
                    );
                }
            }
        }
        for axis in 0..count {
            let angle =
                -std::f32::consts::FRAC_PI_2 + axis as f32 * std::f32::consts::TAU / count as f32;
            ctx.draw_line(
                center.x,
                center.y,
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
                grid,
                1.0,
            );
            if let Some(label) = self.radar_axes.get(axis).map(|axis| axis.label.as_str()) {
                let label_center = Point::new(
                    center.x + (radius + 12.0) * angle.cos(),
                    center.y + (radius + 12.0) * angle.sin(),
                );
                let size = ctx.measure_text(label, 9.0);
                ctx.draw_text(
                    label,
                    Point::new(label_center.x - size.w * 0.5, label_center.y),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
            }
        }
        for (series_index, series) in self.radar_series.iter().enumerate() {
            let points = series
                .data
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    let range = self
                        .radar_axes
                        .get(index)
                        .map(|axis| {
                            let start = if axis.range.start().is_finite() {
                                *axis.range.start()
                            } else {
                                0.0
                            };
                            let end = if axis.range.end().is_finite() {
                                *axis.range.end()
                            } else {
                                100.0
                            };
                            (start, end)
                        })
                        .unwrap_or((0.0, 100.0));
                    let value = if value.value.is_finite() {
                        value.value
                    } else {
                        range.0
                    };
                    let range_min = range.0.min(range.1);
                    let range_max = range.0.max(range.1);
                    let ratio = normalized_ratio(value, range_min, range_max);
                    let a = -std::f32::consts::FRAC_PI_2
                        + index as f32 * std::f32::consts::TAU / count as f32;
                    Point::new(
                        center.x + radius * ratio * a.cos(),
                        center.y + radius * ratio * a.sin(),
                    )
                })
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let color = palette_color(series_index);
            if points.len() >= 3 {
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, points[0].y);
                for point in points.iter().skip(1) {
                    path.line_to(point.x, point.y);
                }
                path.close();
                let fill =
                    Color::from_rgba(color.r, color.g, color.b, (255.0 * self.fill_opacity) as u8);
                ctx.fill_path(&path.build(), fill, FillRule::NonZero);
            } else {
                ctx.fill_circle(points[0].x, points[0].y, 3.0, color);
            }
            for pair in points
                .iter()
                .chain(points.first())
                .collect::<Vec<_>>()
                .windows(2)
            {
                ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, color, 1.5);
            }
        }
    }

    fn heatmap_color(&self, value: f32) -> Color {
        if self.color_stops.len() < 2 {
            return lerp_color(self.color_min, self.color_max, value);
        }
        let value = value.clamp(0.0, 1.0);
        let Some(window) = self
            .color_stops
            .windows(2)
            .find(|window| value <= window[1].0)
        else {
            return self
                .color_stops
                .last()
                .map_or(self.color_max, |(_, color)| *color);
        };
        let span = (window[1].0 - window[0].0).max(f32::EPSILON);
        lerp_color(window[0].1, window[1].1, (value - window[0].0) / span)
    }

    fn paint_heatmap(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Heatmap(cells) = &self.payload else {
            return;
        };
        let leap = is_leap_year(self.year);
        let calendar_days = calendar_day_count(self.year);
        let calendar_offset = january_first_weekday(self.year);
        let calendar = if self.calendar_mode {
            cells
                .iter()
                .filter(|cell| cell.x < calendar_days)
                .map(|cell| {
                    let ordinal = calendar_offset + cell.x;
                    (ordinal / 7, ordinal % 7, cell.value)
                })
                .collect::<Vec<_>>()
        } else {
            cells
                .iter()
                .map(|cell| (cell.x, cell.y, cell.value))
                .collect::<Vec<_>>()
        };
        let max_x = if self.calendar_mode {
            (calendar_offset + calendar_days - 1) / 7 + 1
        } else {
            calendar
                .iter()
                .map(|cell| cell.0)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .min(MAX_HEATMAP_DIMENSION)
        };
        let max_y = if self.calendar_mode {
            7
        } else {
            calendar
                .iter()
                .map(|cell| cell.1)
                .max()
                .unwrap_or(0)
                .saturating_add(1)
                .min(MAX_HEATMAP_DIMENSION)
        };
        let max = cells
            .iter()
            .map(|cell| {
                if cell.value.is_finite() {
                    cell.value
                } else {
                    0.0
                }
            })
            .fold(0.0_f32, f32::max)
            .max(1.0);
        let label_left = if self.y_labels.is_empty() { 0.0 } else { 42.0 };
        let label_bottom = if self.x_labels.is_empty() { 0.0 } else { 20.0 };
        let cells_plot = Rect::new(
            plot.x + label_left,
            plot.y,
            (plot.w - label_left).max(0.0),
            (plot.h - label_bottom).max(0.0),
        );
        let configured = if self.calendar_mode {
            self.cell_size
        } else {
            f32::INFINITY
        };
        let w = ((cells_plot.w - self.cell_gap * max_x as f32) / max_x as f32)
            .min(configured)
            .max(0.0);
        let h = if self.calendar_mode {
            ((cells_plot.h - self.cell_gap * max_y as f32) / max_y as f32)
                .min(configured)
                .max(0.0)
        } else {
            ((cells_plot.h - self.cell_gap * max_y as f32) / max_y as f32).max(0.0)
        };
        for (x, y, value) in &calendar {
            if *x >= max_x || *y >= max_y {
                continue;
            }
            let finite_value = if value.is_finite() { *value } else { 0.0 };
            let t = (finite_value / max).clamp(0.0, 1.0);
            let color = self.heatmap_color(t);
            let rect = Rect::new(
                cells_plot.x + *x as f32 * (w + self.cell_gap),
                cells_plot.y + *y as f32 * (h + self.cell_gap),
                w.max(0.0),
                h.max(0.0),
            );
            ctx.fill_rect(rect, color, None);
            if self.show_values {
                ctx.text_center(&format!("{}", finite_value), rect, Color::white(), 9.0);
            }
        }
        for (index, label) in self.x_labels.iter().take(max_x).enumerate() {
            let rect = Rect::new(
                cells_plot.x + index as f32 * (w + self.cell_gap),
                cells_plot.y + cells_plot.h + 2.0,
                w,
                label_bottom,
            );
            ctx.text_center(label, rect, ctx.tokens().color_text_secondary(), 9.0);
        }
        for (index, label) in self.y_labels.iter().take(max_y).enumerate() {
            let rect = Rect::new(
                plot.x,
                cells_plot.y + index as f32 * (h + self.cell_gap),
                label_left - 4.0,
                h,
            );
            ctx.text_center(label, rect, ctx.tokens().color_text_secondary(), 9.0);
        }
        if self.calendar_mode {
            let month_lengths = [31_usize, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
            let mut day = 0_usize;
            for (month, length) in month_lengths.into_iter().enumerate() {
                let length = if month == 1 && leap { 29 } else { length };
                let week = (calendar_offset + day) / 7;
                let label = format!("{}月", month + 1);
                ctx.draw_text(
                    &label,
                    Point::new(
                        cells_plot.x + week as f32 * (w + self.cell_gap),
                        cells_plot.y - 12.0,
                    ),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
                day += length;
            }
        }
    }

    fn paint_funnel(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Funnel(data) = &self.payload else {
            return;
        };
        let max = data
            .iter()
            .map(|item| {
                if item.value.is_finite() {
                    item.value.max(0.0)
                } else {
                    0.0
                }
            })
            .fold(1.0, f32::max);
        let gap = self
            .funnel_gap
            .min(plot.h / data.len().max(1) as f32)
            .max(0.0);
        let h = (plot.h - gap * data.len().saturating_sub(1) as f32) / data.len().max(1) as f32;
        for (index, item) in data.iter().enumerate() {
            let value = if item.value.is_finite() {
                item.value.max(0.0)
            } else {
                0.0
            };
            let width = plot.w * (value / max).clamp(0.05, 1.0);
            let next_width = data.get(index + 1).map_or(0.05, |item| {
                let value = if item.value.is_finite() {
                    item.value.max(0.0)
                } else {
                    0.0
                };
                (value / max).clamp(0.05, 1.0)
            }) * plot.w;
            let align = if self.funnel_shape == FunnelShape::Symmetric {
                FunnelAlign::Center
            } else {
                self.funnel_align
            };
            let x = match align {
                FunnelAlign::Center => plot.x + (plot.w - width) * 0.5,
                FunnelAlign::Left => plot.x,
                FunnelAlign::Right => plot.x + plot.w - width,
            };
            let y = plot.y + index as f32 * (h + gap);
            let bottom_x = match align {
                FunnelAlign::Center => plot.x + (plot.w - next_width) * 0.5,
                FunnelAlign::Left => plot.x,
                FunnelAlign::Right => plot.x + plot.w - next_width,
            };
            let mut path = PathBuilder::new();
            path.move_to(x, y);
            path.line_to(x + width, y);
            path.line_to(bottom_x + next_width, y + h.max(0.0));
            path.line_to(bottom_x, y + h.max(0.0));
            path.close();
            let funnel_color = ctx.tokens().color_primary();
            ctx.fill_path(&path.build(), funnel_color, FillRule::NonZero);
            if self.label_visible {
                let mut label = format!("{} {}", item.label, value);
                if self.show_conversion_rate && index > 0 {
                    let previous = if data[index - 1].value.is_finite() {
                        data[index - 1].value.abs().max(f32::EPSILON)
                    } else {
                        f32::EPSILON
                    };
                    label.push_str(&format!(" ({:.0}%)", value / previous * 100.0));
                }
                let label_rect = Rect::new(x, y, width, h);
                if self.label_position == LabelPosition::Right {
                    ctx.draw_text(
                        &label,
                        Point::new(x + width + 4.0, y + h * 0.5),
                        ctx.tokens().color_text(),
                        10.0,
                    );
                } else {
                    ctx.text_center(&label, label_rect, Color::white(), 10.0);
                }
            }
        }
    }

    fn paint_waterfall(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Waterfall(data) = &self.payload else {
            return;
        };
        if data.is_empty() {
            return;
        }
        self.paint_axes(ctx, plot);
        let mut ranges = Vec::with_capacity(data.len());
        let mut cumulative = 0.0;
        let mut min_value = 0.0_f32;
        let mut max_value = 0.0_f32;
        for item in data {
            let value = if item.value.is_finite() {
                item.value
            } else {
                0.0
            };
            let start = match item.kind {
                WaterfallKind::Total => 0.0,
                _ => cumulative,
            };
            let end = match item.kind {
                WaterfallKind::Total => value,
                _ => cumulative + value,
            };
            cumulative = end;
            min_value = min_value.min(start.min(end));
            max_value = max_value.max(start.max(end));
            ranges.push((start, end));
        }
        let to_x = |value: f32| plot.x + normalized_ratio(value, min_value, max_value) * plot.w;
        let to_y =
            |value: f32| plot.y + plot.h - normalized_ratio(value, min_value, max_value) * plot.h;
        let category_w = plot.w / data.len() as f32;
        let category_h = plot.h / data.len() as f32;
        let bar_w = category_w * 0.7;
        let bar_h = category_h * 0.7;
        for (index, (item, (start, end))) in data.iter().zip(ranges.iter()).enumerate() {
            let color = match item.kind {
                WaterfallKind::Increase => ctx.tokens().color_success(),
                WaterfallKind::Decrease => ctx.tokens().color_error(),
                WaterfallKind::Total => ctx.tokens().color_primary(),
            };
            let rect = if self.horizontal {
                let x = to_x((*start).min(*end));
                Rect::new(
                    x,
                    plot.y + index as f32 * category_h + category_h * 0.15,
                    (to_x((*start).max(*end)) - x).max(1.0),
                    bar_h.max(1.0),
                )
            } else {
                let y = to_y((*start).max(*end));
                Rect::new(
                    plot.x + index as f32 * category_w + category_w * 0.15,
                    y,
                    bar_w.max(1.0),
                    (to_y((*start).min(*end)) - y).max(1.0),
                )
            };
            ctx.fill_rect(rect, color, None);
            if self.horizontal {
                ctx.draw_text(
                    &item.label,
                    Point::new(rect.x + rect.w + 4.0, rect.y + rect.h * 0.5),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
            } else {
                ctx.text_center(
                    &item.label,
                    Rect::new(rect.x, plot.y + plot.h + 2.0, rect.w, 14.0),
                    ctx.tokens().color_text_secondary(),
                    9.0,
                );
            }
            if let Some((next_start, _)) = ranges.get(index + 1) {
                let connector_color = ctx.tokens().color_border();
                if self.horizontal {
                    let y = rect.y + rect.h;
                    ctx.draw_line(
                        rect.x + rect.w,
                        y,
                        rect.x + rect.w,
                        plot.y + (index + 1) as f32 * category_h + category_h * 0.15,
                        connector_color,
                        1.0,
                    );
                    let _ = next_start;
                } else {
                    let x = rect.x + rect.w;
                    ctx.draw_line(
                        x,
                        to_y(*end),
                        plot.x + (index + 1) as f32 * category_w + category_w * 0.15,
                        to_y(*next_start),
                        connector_color,
                        1.0,
                    );
                }
            }
        }
    }

    fn paint_combo(&self, ctx: &mut PaintContext, plot: Rect) {
        self.paint_axes(ctx, plot);
        if !self.y_axis_right.is_empty() {
            let size = ctx.measure_text(&self.y_axis_right, 10.0);
            ctx.draw_text(
                &self.y_axis_right,
                Point::new(plot.x + plot.w - size.w, plot.y - 2.0),
                ctx.tokens().color_text_secondary(),
                10.0,
            );
        }
        let mut bars: Vec<(String, Vec<BarData>, AxisSide)> = Vec::new();
        let mut lines: Vec<(String, Vec<LineData>, AxisSide, LineStyle, bool)> = Vec::new();
        if !self.combo_series.is_empty() {
            for (index, series) in self.combo_series.iter().enumerate() {
                match series.chart_type {
                    ChartType::Bar => bars.push((
                        series.name.clone(),
                        series
                            .data
                            .iter()
                            .map(|item| BarData::new(&item.label, item.value, palette_color(index)))
                            .collect(),
                        series.axis,
                    )),
                    ChartType::Line | ChartType::Area => lines.push((
                        series.name.clone(),
                        series.data.clone(),
                        series.axis,
                        series.line_style,
                        series.chart_type == ChartType::Area,
                    )),
                }
            }
        } else {
            bars.extend(
                self.bar_series
                    .iter()
                    .map(|series| (series.name.clone(), series.data.clone(), AxisSide::Left)),
            );
            let line_axis = if !self.y_axis_right.is_empty() || !self.bar_series.is_empty() {
                AxisSide::Right
            } else {
                AxisSide::Left
            };
            lines.extend(self.line_series.iter().map(|series| {
                (
                    series.name.clone(),
                    series.data.clone(),
                    line_axis,
                    LineStyle::Solid,
                    false,
                )
            }));
        }
        let count = bars
            .iter()
            .map(|(_, data, _)| data.len())
            .chain(lines.iter().map(|(_, data, _, _, _)| data.len()))
            .max()
            .unwrap_or(0);
        if count == 0 {
            return;
        }
        let range_for = |axis: AxisSide| {
            let mut min = 0.0_f32;
            let mut max = 0.0_f32;
            for (_, data, item_axis) in &bars {
                if *item_axis == axis {
                    for item in data {
                        let value = if item.value.is_finite() {
                            item.value
                        } else {
                            0.0
                        };
                        min = min.min(value);
                        max = max.max(value);
                    }
                }
            }
            for (_, data, item_axis, _, _) in &lines {
                if *item_axis == axis {
                    for item in data {
                        let value = if item.value.is_finite() {
                            item.value
                        } else {
                            0.0
                        };
                        min = min.min(value);
                        max = max.max(value);
                    }
                }
            }
            (min, max.max(min + f32::EPSILON))
        };
        let left_range = range_for(AxisSide::Left);
        let right_range = range_for(AxisSide::Right);
        let to_y = |value: f32, axis: AxisSide| {
            let (min, max) = if axis == AxisSide::Right {
                right_range
            } else {
                left_range
            };
            plot.y + plot.h - normalized_ratio(value, min, max) * plot.h
        };
        let category_w = plot.w / count as f32;
        let bar_count = bars.len().max(1) as f32;
        for (bar_index, (_, data, axis)) in bars.iter().enumerate() {
            for (index, item) in data.iter().enumerate() {
                let value = if item.value.is_finite() {
                    item.value
                } else {
                    0.0
                };
                let w = category_w * (1.0 - self.category_gap);
                let x = plot.x
                    + index as f32 * category_w
                    + category_w * self.category_gap * 0.5
                    + bar_index as f32 * w / bar_count;
                let bar_w = (w / bar_count * (1.0 - self.bar_gap)).max(1.0);
                let y = to_y(value, *axis);
                let baseline = to_y(0.0, *axis);
                let rect = Rect::new(x, y.min(baseline), bar_w, (baseline - y).abs().max(1.0));
                ctx.fill_rect(rect, item.color, None);
                if bar_index == 0 {
                    ctx.text_center(
                        &item.label,
                        Rect::new(x, plot.y + plot.h + 2.0, bar_w, 14.0),
                        ctx.tokens().color_text_secondary(),
                        9.0,
                    );
                }
            }
        }
        for (line_index, (_, data, axis, line_style, area)) in lines.iter().enumerate() {
            let points = data
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    let value = if item.value.is_finite() {
                        item.value
                    } else {
                        0.0
                    };
                    Point::new(
                        if count == 1 {
                            plot.x + plot.w * 0.5
                        } else {
                            plot.x + index as f32 * category_w + category_w * 0.5
                        },
                        to_y(value, *axis),
                    )
                })
                .collect::<Vec<_>>();
            if points.is_empty() {
                continue;
            }
            let color = palette_color(bars.len() + line_index);
            if *area && points.len() >= 2 {
                let baseline = to_y(0.0, *axis);
                let mut path = PathBuilder::new();
                path.move_to(points[0].x, baseline);
                for point in &points {
                    path.line_to(point.x, point.y);
                }
                if let Some(last) = points.last() {
                    path.line_to(last.x, baseline);
                }
                path.close();
                ctx.fill_path(
                    &path.build(),
                    Color::from_rgba(color.r, color.g, color.b, (255.0 * self.fill_opacity) as u8),
                    FillRule::NonZero,
                );
            }
            for pair in points.windows(2) {
                if *line_style == LineStyle::Dashed {
                    for segment in 0..8 {
                        if segment % 2 == 1 {
                            continue;
                        }
                        let start = segment as f32 / 8.0;
                        let end = (segment + 1) as f32 / 8.0;
                        let from = Point::new(
                            pair[0].x + (pair[1].x - pair[0].x) * start,
                            pair[0].y + (pair[1].y - pair[0].y) * start,
                        );
                        let to = Point::new(
                            pair[0].x + (pair[1].x - pair[0].x) * end,
                            pair[0].y + (pair[1].y - pair[0].y) * end,
                        );
                        ctx.draw_line(from.x, from.y, to.x, to.y, color, 2.0);
                    }
                } else {
                    ctx.draw_line(pair[0].x, pair[0].y, pair[1].x, pair[1].y, color, 2.0);
                }
            }
            for point in points {
                ctx.fill_circle(point.x, point.y, self.point_size, color);
            }
        }
    }

    fn paint_treemap(&self, ctx: &mut PaintContext, plot: Rect) {
        let ChartPayload::Treemap(nodes) = &self.payload else {
            return;
        };
        self.paint_treemap_nodes(ctx, nodes, plot, 0);
    }

    fn paint_treemap_nodes(
        &self,
        ctx: &mut PaintContext,
        nodes: &[TreemapNode],
        plot: Rect,
        depth: usize,
    ) {
        if nodes.is_empty() || plot.w <= 0.0 || plot.h <= 0.0 {
            return;
        }
        let total = nodes
            .iter()
            .map(|node| {
                let child_total = node
                    .children
                    .iter()
                    .map(|child| child.value.max(0.0))
                    .sum::<f32>();
                if child_total > 0.0 {
                    child_total
                } else {
                    node.value.max(0.0)
                }
            })
            .sum::<f32>()
            .max(f32::EPSILON);
        let horizontal = depth.is_multiple_of(2);
        let mut cursor = if horizontal { plot.x } else { plot.y };
        for (index, node) in nodes.iter().enumerate() {
            let weight = {
                let child_total = node
                    .children
                    .iter()
                    .map(|child| child.value.max(0.0))
                    .sum::<f32>();
                if child_total > 0.0 {
                    child_total
                } else {
                    node.value.max(0.0)
                }
            };
            let extent = if horizontal {
                plot.w * weight / total
            } else {
                plot.h * weight / total
            };
            let raw = if horizontal {
                Rect::new(cursor, plot.y, extent, plot.h)
            } else {
                Rect::new(plot.x, cursor, plot.w, extent)
            };
            let gap = self.treemap_gap.min(raw.w * 0.5).min(raw.h * 0.5);
            let rect = Rect::new(
                raw.x + gap * 0.5,
                raw.y + gap * 0.5,
                (raw.w - gap).max(0.0),
                (raw.h - gap).max(0.0),
            );
            let color = palette_color(depth * nodes.len() + index);
            ctx.fill_rect(rect, color, None);
            if node.children.is_empty() {
                if self.label_visible {
                    ctx.text_center(&node.label, rect, Color::white(), 11.0);
                }
            } else {
                if self.label_visible {
                    ctx.draw_text(
                        &node.label,
                        Point::new(rect.x + 4.0, rect.y + 12.0),
                        Color::white(),
                        10.0,
                    );
                }
                let inner = Rect::new(
                    rect.x + 2.0,
                    rect.y + 16.0,
                    rect.w - 4.0,
                    (rect.h - 18.0).max(0.0),
                );
                self.paint_treemap_nodes(ctx, &node.children, inner, depth + 1);
            }
            cursor += extent;
        }
    }

    fn paint_gauge(&self, ctx: &mut PaintContext, plot: Rect) {
        let center = Point::new(plot.x + plot.w * 0.5, plot.y + plot.h * 0.56);
        let radius = plot.w.min(plot.h) * 0.38;
        let (start, sweep) = match self.gauge_type {
            GaugeType::Dashboard => (std::f32::consts::PI, std::f32::consts::PI),
            GaugeType::Full | GaugeType::Ring => {
                (-std::f32::consts::FRAC_PI_2, std::f32::consts::TAU)
            }
        };
        let range_min = self.gauge_min.min(self.gauge_max);
        let range_max = self.gauge_min.max(self.gauge_max);
        let ratio = normalized_ratio(self.gauge_value, range_min, range_max);
        let bg = ctx.tokens().color_fill_tertiary();
        ctx.fill_sector(center.x, center.y, radius, start, start + sweep, bg);
        if self.gauge_ranges.is_empty() {
            let color = ctx.tokens().color_primary();
            ctx.fill_sector(
                center.x,
                center.y,
                radius,
                start,
                start + sweep * ratio,
                color,
            );
        } else {
            for segment in &self.gauge_ranges {
                if !segment.start.is_finite() || !segment.end.is_finite() {
                    continue;
                }
                let raw_start = normalized_ratio(segment.start, range_min, range_max);
                let raw_end = normalized_ratio(segment.end, range_min, range_max);
                let segment_start = raw_start.min(raw_end);
                let segment_end = raw_start.max(raw_end);
                let end = segment_end.min(ratio);
                if end > segment_start {
                    ctx.fill_sector(
                        center.x,
                        center.y,
                        radius,
                        start + sweep * segment_start,
                        start + sweep * end,
                        segment.color,
                    );
                }
            }
        }
        if self.gauge_type == GaugeType::Ring {
            ctx.fill_circle(
                center.x,
                center.y,
                radius * 0.62,
                ctx.tokens().color_bg_container(),
            );
        }
        if self.pointer_width > 0.0 {
            let angle = start + sweep * ratio;
            let pointer_color = self.pointer_color.unwrap_or(ctx.tokens().color_text());
            ctx.draw_line(
                center.x,
                center.y,
                center.x + radius * 0.9 * angle.cos(),
                center.y + radius * 0.9 * angle.sin(),
                pointer_color,
                self.pointer_width,
            );
            ctx.fill_circle(center.x, center.y, self.pointer_width * 1.5, pointer_color);
        }
        let label = self
            .value_format
            .as_ref()
            .map(|format| format(self.gauge_value))
            .unwrap_or_else(|| format!("{:.0}", self.gauge_value));
        ctx.text_center(
            &label,
            Rect::new(center.x - radius, center.y - 12.0, radius * 2.0, 24.0),
            ctx.tokens().color_text(),
            16.0,
        );
    }

    pub(crate) fn sync_from(&mut self, mut next: Self) {
        let last_frame = self.last_frame.get();
        let hovered_pos = self.hovered_pos.get();
        let tooltip_pos = self.tooltip_pos.get();
        let pan_offset = self.pan_offset.get();
        let zoom = self.zoom.get();
        let same_animation = self.animation_config == next.animation_config;
        let animation_player = if same_animation {
            self.animation_player.take()
        } else {
            next.animation_player.take()
        };
        let animation_dirty = if same_animation {
            self.animation_dirty.get()
        } else {
            next.animation_dirty.get()
        };
        let tracks_pointer = next.interaction.is_some()
            || next.brush_config.is_some()
            || next.tooltip_config.is_some();
        let keeps_pan = next.interaction.as_ref().is_some_and(|config| config.pan);
        let keeps_zoom = next.interaction.as_ref().is_some_and(|config| config.zoom);
        let keeps_click_tooltip = next
            .tooltip_config
            .as_ref()
            .is_some_and(|config| config.trigger_mode() == TooltipTrigger::Click);

        *self = next;
        self.last_frame.set(last_frame);
        self.hovered_pos
            .set(tracks_pointer.then_some(hovered_pos).flatten());
        self.tooltip_pos
            .set(keeps_click_tooltip.then_some(tooltip_pos).flatten());
        self.brush_start.set(None);
        self.pan_start.set(None);
        self.pan_origin.set(0.0);
        self.pan_offset
            .set(if keeps_pan { pan_offset } else { 0.0 });
        self.zoom.set(if keeps_zoom { zoom } else { 1.0 });
        self.animation_player = animation_player;
        self.animation_dirty.set(animation_dirty);
    }

    #[cfg(test)]
    pub(crate) fn kind_for_test(&self) -> &'static str {
        match self.kind {
            ChartKind::Generic => "generic",
            ChartKind::Bar => "bar",
            ChartKind::Line => "line",
            ChartKind::Area => "area",
            ChartKind::Scatter => "scatter",
            ChartKind::Radar => "radar",
            ChartKind::Heatmap => "heatmap",
            ChartKind::Funnel => "funnel",
            ChartKind::Waterfall => "waterfall",
            ChartKind::Combo => "combo",
            ChartKind::Treemap => "treemap",
            ChartKind::Gauge => "gauge",
        }
    }

    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        self.tooltip_text_at(pos, frame)
    }

    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }

    #[cfg(test)]
    pub(crate) fn legend_layout_for_test(&self, frame: Rect) -> Option<(Rect, Rect)> {
        let mut plot = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            plot.y += 20.0;
            plot.h = (plot.h - 20.0).max(0.0);
        }
        if !self.subtitle.is_empty() {
            plot.y += 16.0;
            plot.h = (plot.h - 16.0).max(0.0);
        }
        let (plot, legend) = self.legend_layout(plot);
        legend.map(|legend| (plot, legend))
    }

    #[cfg(test)]
    pub(crate) fn animation_progress_for_test(&self) -> Option<f32> {
        self.animation_player
            .as_ref()
            .map(|player| player.opacity_progress)
    }
}

impl Default for ChartPlaceholder {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::ui::view::View for ChartPlaceholder {
    fn build(self) -> crate::ui::view::ViewNode {
        if !self.has_data() {
            if let Some(empty) = crate::ui::config::render_empty_for::<Self>() {
                return empty;
            }
            return crate::ui::view::ViewNode::leaf(crate::ui::widgets::display::Empty::new());
        }
        crate::ui::view::ViewNode::leaf(self)
    }
}

macro_rules! chart_entry {
    ($name:ident, $kind:ident) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl $name {
            #[allow(
                clippy::new_ret_no_self,
                reason = "the public chart entry selects a kind on the shared chart component"
            )]
            pub fn new() -> ChartPlaceholder {
                ChartPlaceholder::new_with_kind(ChartKind::$kind)
            }
        }
    };
}

chart_entry!(AreaChart, Area);
chart_entry!(ScatterChart, Scatter);
chart_entry!(RadarChart, Radar);
chart_entry!(Heatmap, Heatmap);
chart_entry!(FunnelChart, Funnel);
chart_entry!(WaterfallChart, Waterfall);
chart_entry!(ComboChart, Combo);
chart_entry!(Treemap, Treemap);
chart_entry!(Gauge, Gauge);

pub(super) fn catmull_rom_points(points: &[Point], subdivisions: usize) -> Vec<Point> {
    if points.len() < 2 {
        return points.to_vec();
    }
    let subdivisions = subdivisions.clamp(1, 32);
    let mut output = Vec::with_capacity((points.len() - 1) * subdivisions + 1);
    output.push(points[0]);
    for index in 0..points.len() - 1 {
        let p0 = points[index.saturating_sub(1)];
        let p1 = points[index];
        let p2 = points[index + 1];
        let p3 = points[(index + 2).min(points.len() - 1)];
        for step in 1..=subdivisions {
            let t = step as f32 / subdivisions as f32;
            let t2 = t * t;
            let t3 = t2 * t;
            let interpolate = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b
                    + (-a + c) * t
                    + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2
                    + (-a + 3.0 * b - 3.0 * c + d) * t3)
            };
            output.push(Point::new(
                interpolate(p0.x, p1.x, p2.x, p3.x),
                interpolate(p0.y, p1.y, p2.y, p3.y),
            ));
        }
    }
    output
}

fn lerp_color(start: Color, end: Color, t: f32) -> Color {
    let t = t.clamp(0.0, 1.0);
    Color::from_rgba(
        (start.r as f32 + (end.r as f32 - start.r as f32) * t).round() as u8,
        (start.g as f32 + (end.g as f32 - start.g as f32) * t).round() as u8,
        (start.b as f32 + (end.b as f32 - start.b as f32) * t).round() as u8,
        (start.a as f32 + (end.a as f32 - start.a as f32) * t).round() as u8,
    )
}

fn format_number(value: f32) -> String {
    if !value.is_finite() {
        return "0".to_owned();
    }
    if value == value.trunc() {
        format!("{value:.0}")
    } else {
        format!("{value:.2}")
            .trim_end_matches('0')
            .trim_end_matches('.')
            .to_owned()
    }
}

fn format_optional_number(value: Option<f32>) -> String {
    value.map(format_number).unwrap_or_default()
}

fn finite_or_zero(value: f32) -> f32 {
    if value.is_finite() {
        value
    } else {
        0.0
    }
}

pub(super) fn normalized_ratio(value: f32, first: f32, second: f32) -> f32 {
    let min = f64::from(first.min(second));
    let max = f64::from(first.max(second));
    let span = max - min;
    if span <= f64::EPSILON {
        return 0.0;
    }
    ((f64::from(value) - min) / span).clamp(0.0, 1.0) as f32
}

fn normalized_grid_index(value: f32, start: f32, extent: f32, count: usize) -> usize {
    if count == 0 || !value.is_finite() || !start.is_finite() || extent <= 0.0 {
        return 0;
    }
    (((value - start) / extent).clamp(0.0, 1.0 - f32::EPSILON) * count as f32).floor() as usize
}

fn january_first_weekday(year: i32) -> usize {
    if year == 0 {
        return 0;
    }
    let year = i64::from(year) - 1;
    (year + year.div_euclid(4) - year.div_euclid(100) + year.div_euclid(400) + 1).rem_euclid(7)
        as usize
}

fn is_leap_year(year: i32) -> bool {
    year != 0 && (year % 4 == 0 && year % 100 != 0 || year % 400 == 0)
}

fn calendar_day_count(year: i32) -> usize {
    if is_leap_year(year) {
        366
    } else {
        365
    }
}

fn scatter_tooltip_datum(item: &ScatterData) -> TooltipDatum {
    TooltipDatum {
        label: item.label.clone(),
        x: Some(finite_or_zero(item.x)),
        y: Some(finite_or_zero(item.y)),
        ..TooltipDatum::default()
    }
}

fn palette_color(index: usize) -> Color {
    const COLORS: [[u8; 3]; 6] = [
        [22, 119, 255],
        [82, 196, 26],
        [250, 173, 20],
        [245, 34, 45],
        [114, 46, 209],
        [19, 194, 194],
    ];
    let [r, g, b] = COLORS[index % COLORS.len()];
    Color::from_rgba(r, g, b, 255)
}
