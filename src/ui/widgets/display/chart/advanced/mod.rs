//! 高级图表组件：面积、散点、气泡、雷达、热力、漏斗、瀑布、
//! 组合、树图与仪表盘，共用 `ChartPlaceholder` 合成组件。
//!
//! 拆分为子模块：`widget`（组件宏块）、`builder`（构造方法）、
//! `helpers`（数据与交互）、`paint_series` / `paint_advanced`（绘制）。

use std::fmt;
use std::rc::Rc;

use crate::core::Point;
use crate::draw::Color;
use crate::ui::SnapshotFields;

use super::bar_chart::BarData;
use super::line_chart::LineData;

mod builder;
mod helpers;
mod widget;
// 将热力图色阶插值拆为无状态绘制辅助，不引入新的图表状态 owner。
mod heatmap_color;
mod paint_advanced;
mod paint_series;

pub use widget::*;

#[derive(Debug, Clone, PartialEq)]
/// 图表系列：系列名称 + 数据。
pub struct ChartSeries<T> {
    /// 系列名称，显示在图例与 tooltip 中。
    pub name: String,
    /// 系列数据（柱状/折线/散点等，取决于图表类型）。
    pub data: T,
}

impl<T> ChartSeries<T> {
    /// 构造系列。
    pub fn new(name: impl Into<String>, data: T) -> Self {
        Self {
            name: name.into(),
            data,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 图例显示位置。
pub enum LegendPosition {
    /// 顶部。
    Top,
    /// 底部。
    Bottom,
    /// 左侧。
    Left,
    /// 右侧。
    Right,
    /// 不显示。
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 玫瑰图样式。
pub enum RoseStyle {
    /// 半径模式：扇区半径按数值占比。
    Radius,
    /// 面积模式：扇区面积按数值占比。
    Area,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 数据标签位置。
pub enum LabelPosition {
    /// 标签位于图形内部。
    Inside,
    /// 标签位于图形外部。
    Outside,
    /// 标签位于图形右侧。
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 数据点样式。
pub enum PointStyle {
    /// 圆形点。
    Circle,
    /// 菱形点。
    Diamond,
    /// 十字点。
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 雷达图外框形状。
pub enum RadarShape {
    /// 多边形网格。
    Polygon,
    /// 圆形网格。
    Circle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 漏斗图形状。
pub enum FunnelShape {
    /// 常规梯形漏斗。
    Normal,
    /// 对称漏斗（两侧对称收窄）。
    Symmetric,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 漏斗图对齐方式。
pub enum FunnelAlign {
    /// 居中对齐。
    Center,
    /// 左对齐。
    Left,
    /// 右对齐。
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 组合图系列类型。
pub enum ChartType {
    /// 柱状系列。
    Bar,
    /// 折线系列。
    Line,
    /// 面积系列。
    Area,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 组合图系列绑定的坐标轴。
pub enum AxisSide {
    /// 左轴。
    Left,
    /// 右轴。
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 组合图折线样式。
pub enum LineStyle {
    /// 实线。
    Solid,
    /// 虚线。
    Dashed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 仪表盘类型。
pub enum GaugeType {
    /// 半圆仪表盘。
    Dashboard,
    /// 整圆仪表盘。
    Full,
    /// 环形仪表盘。
    Ring,
}

#[derive(Debug, Clone, Copy)]
/// 图表交互配置。
pub struct InteractionConfig {
    /// 是否允许滚轮缩放。
    pub zoom: bool,
    /// 是否允许拖拽平移。
    pub pan: bool,
    /// 是否显示十字参考线。
    pub crosshair: bool,
    /// 点击图表数据项时的回调（参数为数据项标签）。
    pub on_click: Option<fn(&str)>,
}

#[derive(Debug, Clone, Copy)]
/// 图表刷选（范围选择）配置。
pub struct BrushConfig {
    /// 是否启用刷选。
    pub enabled: bool,
    /// 刷选完成回调，参数为选区坐标范围。
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
    /// 数据项标签。
    pub label: String,
    /// 所属系列名称。
    pub series: Option<String>,
    /// 数值。
    pub value: Option<f32>,
    /// 横坐标。
    pub x: Option<f32>,
    /// 纵坐标。
    pub y: Option<f32>,
    /// 占比（0~1）。
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
    /// 构造默认 tooltip 配置。
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置触发方式。
    pub fn trigger(mut self, trigger: TooltipTrigger) -> Self {
        self.trigger = trigger;
        self
    }

    /// 设置文本模板（支持 `{label}`、`{series}`、`{value}`、`{x}`、`{y}`、`{percentage}`）。
    pub fn template(mut self, template: impl Into<String>) -> Self {
        self.template = Some(template.into());
        self
    }

    /// 设置自定义渲染器（优先级高于模板）。
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
/// 散点图数据项。
pub struct ScatterData {
    /// 数据项标签。
    pub label: String,
    /// 横坐标。
    pub x: f32,
    /// 纵坐标。
    pub y: f32,
}

impl ScatterData {
    /// 构造散点数据项。
    pub fn new(label: impl Into<String>, x: f32, y: f32) -> Self {
        Self {
            label: label.into(),
            x,
            y,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 气泡图数据项。
pub struct BubbleData {
    /// 数据项标签。
    pub label: String,
    /// 横坐标。
    pub x: f32,
    /// 纵坐标。
    pub y: f32,
    /// 气泡大小。
    pub size: f32,
}

impl BubbleData {
    /// 构造气泡数据项。
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
/// 雷达图轴定义。
pub struct RadarAxis {
    /// 轴标签。
    pub label: String,
    /// 轴取值范围。
    pub range: std::ops::RangeInclusive<f32>,
}

impl RadarAxis {
    /// 构造雷达图轴。
    pub fn new(label: impl Into<String>, range: std::ops::RangeInclusive<f32>) -> Self {
        Self {
            label: label.into(),
            range,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// 雷达图数据点。
pub struct RadarData {
    /// 数值。
    pub value: f32,
}

impl RadarData {
    /// 构造雷达图数据点。
    pub fn new(value: f32) -> Self {
        Self { value }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// 热力图单元格。
pub struct HeatmapCell {
    /// 列索引。
    pub x: usize,
    /// 行索引。
    pub y: usize,
    /// 单元格数值。
    pub value: f32,
}

impl HeatmapCell {
    /// 构造热力图单元格。
    pub fn new(x: usize, y: usize, value: f32) -> Self {
        Self { x, y, value }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 漏斗图数据项。
pub struct FunnelData {
    /// 数据项标签。
    pub label: String,
    /// 数据项数值。
    pub value: f32,
}

impl FunnelData {
    /// 构造漏斗图数据项。
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// 瀑布图柱类型。
pub enum WaterfallKind {
    /// 合计柱。
    Total,
    /// 增长柱。
    Increase,
    /// 减少柱。
    Decrease,
}

#[derive(Debug, Clone, PartialEq)]
/// 瀑布图数据项。
pub struct WaterfallData {
    /// 数据项标签。
    pub label: String,
    /// 数据项数值。
    pub value: f32,
    /// 柱类型。
    pub kind: WaterfallKind,
}

impl WaterfallData {
    /// 构造瀑布图数据项。
    pub fn new(label: impl Into<String>, value: f32, kind: WaterfallKind) -> Self {
        Self {
            label: label.into(),
            value,
            kind,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 组合图系列。
pub struct ComboSeries<T> {
    /// 系列名称。
    pub name: String,
    /// 系列数据。
    pub data: T,
    /// 系列图表类型。
    pub chart_type: ChartType,
    /// 系列绑定的坐标轴。
    pub axis: AxisSide,
    /// 系列线型。
    pub line_style: LineStyle,
}

impl<T> ComboSeries<T> {
    /// 构造组合图系列（默认折线、左轴、实线）。
    pub fn new(name: impl Into<String>, data: T) -> Self {
        Self {
            name: name.into(),
            data,
            chart_type: ChartType::Line,
            axis: AxisSide::Left,
            line_style: LineStyle::Solid,
        }
    }

    /// 设置系列图表类型。
    pub fn chart_type(mut self, chart_type: ChartType) -> Self {
        self.chart_type = chart_type;
        self
    }

    /// 设置系列绑定的坐标轴。
    pub fn y_axis(mut self, axis: AxisSide) -> Self {
        self.axis = axis;
        self
    }

    /// 设置系列线型。
    pub fn line_style(mut self, line_style: LineStyle) -> Self {
        self.line_style = line_style;
        self
    }
}

#[derive(Debug, Clone, PartialEq)]
/// 树图节点。
pub struct TreemapNode {
    /// 节点标签。
    pub label: String,
    /// 节点数值。
    pub value: f32,
    /// 子节点。
    pub children: Vec<Self>,
}

impl TreemapNode {
    /// 构造树图叶子节点。
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
            children: Vec::new(),
        }
    }

    /// 设置子节点。
    pub fn children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
/// 仪表盘色带范围。
pub struct GaugeRange {
    /// 起始值。
    pub start: f32,
    /// 结束值。
    pub end: f32,
    /// 色带颜色。
    pub color: Color,
}

impl GaugeRange {
    /// 构造色带范围。
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
            if let Some(empty) = crate::ui::widget_runtime::config::render_empty_for::<Self>() {
                return empty;
            }
            return crate::ui::view::ViewNode::leaf(crate::ui::widgets::display::Empty::new());
        }
        crate::ui::view::ViewNode::leaf(self)
    }
}

macro_rules! chart_entry {
    ($name:ident, $kind:ident) => {
        #[doc = concat!("按类型选择共享图表组件的入口：", stringify!($kind), " 图表。")]
        #[derive(Debug, Clone, Copy)]
        pub struct $name;

        impl $name {
            #[allow(
                clippy::new_ret_no_self,
                reason = "the public chart entry selects a kind on the shared chart widget"
            )]
            #[doc = concat!("构造 ", stringify!($kind), " 图表组件。")]
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
    let mut output = Vec::new();
    catmull_rom_points_into(points, subdivisions, &mut output);
    output
}

// 把平滑曲线采样写入调用方复用缓冲，避免动态图表逐帧重新分配。
pub(super) fn catmull_rom_points_into(
    points: &[Point],
    subdivisions: usize,
    output: &mut Vec<Point>,
) {
    output.clear();
    if points.len() < 2 {
        output.extend_from_slice(points);
        return;
    }
    let subdivisions = subdivisions.clamp(1, 32);
    output.reserve((points.len() - 1) * subdivisions + 1);
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
    if value.is_finite() { value } else { 0.0 }
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
    if is_leap_year(year) { 366 } else { 365 }
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
    // 图表系列调色板：固定品牌色序列（首色与 token color_primary 默认值 22,119,255 一致），
    // 属数据可视化色板，与主题解耦、不随换肤变化，故保留字面量。
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
