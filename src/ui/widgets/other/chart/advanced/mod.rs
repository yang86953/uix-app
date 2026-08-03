//! 高级图表组件：面积、散点、气泡、雷达、热力、漏斗、瀑布、
//! 组合、树图与仪表盘，共用 `ChartPlaceholder` 合成组件。
//!
//! 拆分为子模块：`component`（组件宏块）、`builder`（构造方法）、
//! `helpers`（数据与交互）、`paint_series` / `paint_advanced`（绘制）。

use std::fmt;
use std::rc::Rc;

use crate::core::Point;
use crate::draw::Color;
use crate::ui::SnapshotFields;

use super::bar_chart::BarData;
use super::line_chart::LineData;

mod builder;
mod component;
mod helpers;
mod paint_advanced;
mod paint_series;

pub use component::*;

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

pub(crate) enum ChartKind {
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


pub(crate) enum ChartPayload {
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



impl Default for ChartPlaceholder {
    fn default() -> Self {
        Self::new()
    }
}

impl ChartPlaceholder {
    /// Human-readable chart kind label used for the semantic image name
    /// (mirrors the BarChart / LineChart / PieChart snapshot naming).
    fn kind_label(&self) -> &'static str {
        match self.kind {
            ChartKind::Generic => "Chart",
            ChartKind::Bar => "Bar chart",
            ChartKind::Line => "Line chart",
            ChartKind::Area => "Area chart",
            ChartKind::Scatter => "Scatter chart",
            ChartKind::Radar => "Radar chart",
            ChartKind::Heatmap => "Heatmap chart",
            ChartKind::Funnel => "Funnel chart",
            ChartKind::Waterfall => "Waterfall chart",
            ChartKind::Combo => "Combo chart",
            ChartKind::Treemap => "Treemap chart",
            ChartKind::Gauge => "Gauge chart",
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::ChartPlaceholder {
            title: self.title.clone(),
            subtitle: self.subtitle.clone(),
            kind_name: self.kind_label(),
        }
    }
}

impl crate::ui::view::View for ChartPlaceholder {
    fn build(self) -> crate::ui::view::ViewNode {
        if !self.has_data() {
            if let Some(empty) = crate::ui::component::config::render_empty_for::<Self>() {
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

