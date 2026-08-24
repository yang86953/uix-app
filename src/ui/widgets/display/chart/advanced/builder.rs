use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

use crate::draw::Color;
use crate::ui::animation::{AnimationConfig, TransitionPlayer};

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    ADVANCED_CHART_VISUAL_REF, BrushConfig, BubbleData, ChartKind, ChartPayload, ChartPlaceholder,
    ChartSeries, ChartType, ComboSeries, FunnelAlign, FunnelData, FunnelShape, GaugeRange,
    GaugeType, HeatmapCell, InteractionConfig, LabelPosition, LegendPosition, LineStyle,
    PointStyle, RadarAxis, RadarData, RadarShape, RoseStyle, ScatterData, TooltipConfig,
    TreemapNode, WaterfallData, palette_color,
};

impl ChartPlaceholder {
    /// 创建默认图表占位组件（通用类型、默认尺寸与配色）。
    pub fn new() -> Self {
        // 所有视觉默认值只读取同目录 UIX 生成项。
        let visual = ADVANCED_CHART_VISUAL_REF;
        let defaults = visual.defaults;
        Self {
            width: defaults.width,
            height: defaults.height,
            kind: ChartKind::Generic,
            payload: ChartPayload::Empty,
            background: None,
            padding: defaults.padding,
            legend: defaults.legend,
            title: String::new(),
            subtitle: String::new(),
            responsive: false,
            grouped: false,
            stacked: false,
            horizontal: false,
            smooth: false,
            step: false,
            bar_gap: defaults.bar_gap,
            category_gap: defaults.category_gap,
            rose: false,
            rose_style: defaults.rose_style,
            start_angle: defaults.start_angle,
            end_angle: defaults.end_angle,
            total: None,
            label_visible: defaults.label_visible,
            label_position: defaults.label_position,
            x_axis: String::new(),
            y_axis: String::new(),
            y_axis_right: String::new(),
            bubble_scale: defaults.bubble_scale,
            point_size: defaults.point_size,
            point_style: defaults.point_style,
            grid_levels: defaults.grid_levels,
            fill_opacity: defaults.fill_opacity,
            color_min: defaults.heatmap_color_min,
            color_max: defaults.heatmap_color_max,
            calendar_mode: false,
            year: 0,
            cell_size: defaults.cell_size,
            cell_gap: defaults.cell_gap,
            show_values: false,
            show_conversion_rate: false,
            funnel_align: defaults.funnel_align,
            funnel_shape: defaults.funnel_shape,
            funnel_gap: defaults.funnel_gap,
            treemap_gap: defaults.treemap_gap,
            bar_series: Vec::new(),
            line_series: Vec::new(),
            combo_series: Vec::new(),
            scatter_series: Vec::new(),
            radar_axes: Vec::new(),
            radar_series: Vec::new(),
            radar_shape: defaults.radar_shape,
            gauge_ranges: Vec::new(),
            gauge_value: 0.0,
            gauge_min: defaults.gauge_min,
            gauge_max: defaults.gauge_max,
            gauge_type: defaults.gauge_type,
            pointer_width: defaults.pointer_width,
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
            zoom: Cell::new(defaults.zoom),
            visual,
        }
    }

    /// 以指定图表类型创建图表（供内部/测试使用）。
    pub(crate) fn new_with_kind(kind: ChartKind) -> Self {
        let mut chart = Self::new();
        chart.kind = kind;
        chart
    }

    /// 按类型装载数据载荷：按数据实际类型自动识别并设置图表类型。
    pub fn data<T: 'static>(mut self, data: T) -> Self {
        let mut any: Box<dyn Any> = Box::new(data);
        macro_rules! take_payload {
            // 图表载荷与类型标记采用 Rust 2024 表达式片段语义。
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
        // 折线数据：若已显式声明面积图则保持类型不变。
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
        // 类型无法识别时保持空载荷。
        self.payload = ChartPayload::Empty;
        self
    }

    /// 装载系列数据：支持柱/线/雷达/散点/组合系列，自动设置类型。
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
                // 未显式声明类型时才回退为折线图。
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
        // 组合系列：按 chart_type 拆分到柱/线序列，并赋调色板颜色。
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
    /// 设置图表宽度（非负）。
    pub fn width(mut self, width: f32) -> Self {
        self.width = width.max(0.0);
        self
    }
    /// 设置图表高度（非负）。
    pub fn height(mut self, height: f32) -> Self {
        self.height = height.max(0.0);
        self
    }
    /// 同时设置宽高。
    pub fn size(mut self, size: f32) -> Self {
        self.width = size.max(0.0);
        self.height = size.max(0.0);
        self
    }
    /// 开启分组（并列）柱状图。
    pub fn grouped(mut self, value: bool) -> Self {
        self.grouped = value;
        self
    }
    /// 开启堆叠模式。
    pub fn stacked(mut self, value: bool) -> Self {
        self.stacked = value;
        self
    }
    /// 开启横向布局。
    pub fn horizontal(mut self, value: bool) -> Self {
        self.horizontal = value;
        self
    }
    /// 设置柱间间隙比例（0~0.9）。
    pub fn bar_gap(mut self, value: f32) -> Self {
        self.bar_gap = if value.is_finite() {
            value.clamp(0.0, 0.9)
        } else {
            0.1
        };
        self
    }
    /// 设置分类间隙比例（0~0.9）。
    pub fn category_gap(mut self, value: f32) -> Self {
        self.category_gap = if value.is_finite() {
            value.clamp(0.0, 0.9)
        } else {
            0.2
        };
        self
    }
    /// 设置图例位置。
    pub fn legend(mut self, value: LegendPosition) -> Self {
        self.legend = value;
        self
    }
    /// 开启平滑曲线。
    pub fn smooth(mut self, value: bool) -> Self {
        self.smooth = value;
        self
    }
    /// 开启阶梯折线。
    pub fn step(mut self, value: bool) -> Self {
        self.step = value;
        self
    }
    /// 开启玫瑰图（柱状图变体）。
    pub fn rose(mut self, value: bool) -> Self {
        self.rose = value;
        // 玫瑰图以柱状图为基底。
        if value {
            self.kind = ChartKind::Bar;
        }
        self
    }
    /// 设置玫瑰图样式。
    pub fn rose_style(mut self, value: RoseStyle) -> Self {
        self.rose_style = value;
        self
    }
    /// 设置起始角度（仅有限值生效）。
    pub fn start_angle(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.start_angle = value;
        }
        self
    }
    /// 设置结束角度（仅有限值生效）。
    pub fn end_angle(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.end_angle = value;
        }
        self
    }
    /// 设置数据总量（用于占比计算；仅有限值生效）。
    pub fn total(mut self, value: f32) -> Self {
        self.total = value.is_finite().then_some(value.max(0.0));
        self
    }
    /// 开关数据标签显示。
    pub fn label_visible(mut self, value: bool) -> Self {
        self.label_visible = value;
        self
    }
    /// 设置数据标签位置。
    pub fn label_position(mut self, value: LabelPosition) -> Self {
        self.label_position = value;
        self
    }
    /// 设置 x 轴标题。
    pub fn x_axis(mut self, value: impl Into<String>) -> Self {
        self.x_axis = value.into();
        self
    }
    /// 设置 y 轴标题。
    pub fn y_axis(mut self, value: impl Into<String>) -> Self {
        self.y_axis = value.into();
        self
    }
    /// 设置气泡缩放系数（非负）。
    pub fn bubble_scale(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.bubble_scale = value.max(0.0);
        }
        self
    }
    /// 设置散点大小（非负）。
    pub fn point_size(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.point_size = value.max(0.0);
        }
        self
    }
    /// 设置散点样式。
    pub fn point_style(mut self, value: PointStyle) -> Self {
        self.point_style = value;
        self
    }
    /// 设置雷达轴（自动切到雷达图）。
    pub fn axes<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(axes) = any.downcast::<Vec<RadarAxis>>() {
            self.radar_axes = *axes;
            self.kind = ChartKind::Radar;
        }
        self
    }
    /// 设置图形形状：雷达多边形/圆形或漏斗形状（自动切类型）。
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
    /// 设置网格层级数（1~64）。
    pub fn grid_levels(mut self, value: usize) -> Self {
        self.grid_levels = value.clamp(1, 64);
        self
    }
    /// 设置填充不透明度（0~1）；折线图开启后自动转为面积图。
    pub fn fill_opacity(mut self, value: f32) -> Self {
        self.fill_opacity = if value.is_finite() {
            value.clamp(0.0, 1.0)
        } else {
            0.25
        };
        // 通用/折线类型开启填充即视为面积图。
        if matches!(self.kind, ChartKind::Generic | ChartKind::Line) {
            self.kind = ChartKind::Area;
        }
        self
    }
    /// 设置 x 轴标签（支持 `Vec<String>` 与 `Vec<&str>`；自动切到热力图）。
    pub fn x_labels<T: 'static>(mut self, _value: T) -> Self {
        let value: Box<dyn Any> = Box::new(_value);
        let value = match value.downcast::<Vec<String>>() {
            Ok(labels) => {
                self.x_labels = *labels;
                None
            }
            Err(value) => Some(value),
        };
        // 兼容 &'static str 标签列表。
        if let Some(value) = value {
            if let Ok(labels) = value.downcast::<Vec<&'static str>>() {
                self.x_labels = labels.iter().map(|label| (*label).to_owned()).collect();
            }
        }
        self.kind = ChartKind::Heatmap;
        self
    }
    /// 设置 y 轴标签（支持 `Vec<String>` 与 `Vec<&str>`；自动切到热力图）。
    pub fn y_labels<T: 'static>(mut self, _value: T) -> Self {
        let value: Box<dyn Any> = Box::new(_value);
        let value = match value.downcast::<Vec<String>>() {
            Ok(labels) => {
                self.y_labels = *labels;
                None
            }
            Err(value) => Some(value),
        };
        // 兼容 &'static str 标签列表。
        if let Some(value) = value {
            if let Ok(labels) = value.downcast::<Vec<&'static str>>() {
                self.y_labels = labels.iter().map(|label| (*label).to_owned()).collect();
            }
        }
        self.kind = ChartKind::Heatmap;
        self
    }
    /// 设置热力图渐变色两端。
    pub fn color_range(mut self, min: Color, max: Color) -> Self {
        self.color_min = min;
        self.color_max = max;
        self
    }
    /// 设置色阶（支持 `Vec<(f32, Color)>` 或 `Vec<Color>`，自动排序）。
    pub fn color_stops<T: 'static>(mut self, value: T) -> Self {
        let value: Box<dyn Any> = Box::new(value);
        let value = match value.downcast::<Vec<(f32, Color)>>() {
            Ok(stops) => {
                // 过滤非法位置并按位置升序排列。
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
        // 纯色列表按索引均匀分布。
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
    /// 开启日历模式（自动切到热力图）。
    pub fn calendar_mode(mut self, value: bool) -> Self {
        self.calendar_mode = value;
        self.kind = ChartKind::Heatmap;
        self
    }
    /// 设置日历模式年份。
    pub fn year(mut self, value: i32) -> Self {
        self.year = value;
        self
    }
    /// 设置单元格尺寸（≥1）。
    pub fn cell_size(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.cell_size = value.max(1.0);
        }
        self
    }
    /// 设置单元格间隙（≥0）。
    pub fn cell_gap(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.cell_gap = value.max(0.0);
        }
        self
    }
    /// 开关热力图单元格数值显示。
    pub fn show_values(mut self, value: bool) -> Self {
        self.show_values = value;
        self
    }
    /// 开关漏斗转化率显示。
    pub fn show_conversion_rate(mut self, value: bool) -> Self {
        self.show_conversion_rate = value;
        self
    }
    /// 设置漏斗对齐方式。
    pub fn align(mut self, value: FunnelAlign) -> Self {
        self.funnel_align = value;
        self
    }
    /// 同时设置漏斗/矩形树间隙。
    pub fn gap(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.funnel_gap = value.max(0.0);
            self.treemap_gap = value.max(0.0);
        }
        self
    }
    /// 装载柱系列（自动切到组合图）。
    pub fn bar_series<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(series) = any.downcast::<Vec<ChartSeries<Vec<BarData>>>>() {
            self.bar_series = *series;
            self.kind = ChartKind::Combo;
        }
        self
    }
    /// 装载线系列（自动切到组合图）。
    pub fn line_series<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(series) = any.downcast::<Vec<ChartSeries<Vec<LineData>>>>() {
            self.line_series = *series;
            self.kind = ChartKind::Combo;
        }
        self
    }
    /// 设置左侧 y 轴标题（同 y_axis）。
    pub fn y_axis_left(mut self, value: impl Into<String>) -> Self {
        self.y_axis = value.into();
        self
    }
    /// 设置右侧 y 轴标题。
    pub fn y_axis_right(mut self, value: impl Into<String>) -> Self {
        self.y_axis_right = value.into();
        self
    }
    /// 添加一条参考线（仅有限值生效）。
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
    /// 设置仪表盘当前值（自动切到仪表盘）。
    pub fn value(mut self, value: f32) -> Self {
        self.gauge_value = if value.is_finite() { value } else { 0.0 };
        self.kind = ChartKind::Gauge;
        self
    }
    /// 设置仪表盘最小值。
    pub fn min(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.gauge_min = value;
        }
        self
    }
    /// 设置仪表盘最大值。
    pub fn max(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.gauge_max = value;
        }
        self
    }
    /// 设置仪表盘分段颜色（自动切到仪表盘）。
    pub fn range_colors<T: 'static>(mut self, value: T) -> Self {
        let any: Box<dyn Any> = Box::new(value);
        if let Ok(ranges) = any.downcast::<Vec<GaugeRange>>() {
            self.gauge_ranges = *ranges;
            self.kind = ChartKind::Gauge;
        }
        self
    }
    /// 设置图表标题。
    pub fn title(mut self, value: impl Into<String>) -> Self {
        self.title = value.into();
        self
    }
    /// 设置图表副标题。
    pub fn subtitle(mut self, value: impl Into<String>) -> Self {
        self.subtitle = value.into();
        self
    }
    /// 设置仪表盘类型（自动切到仪表盘）。
    pub fn gauge_type(mut self, value: GaugeType) -> Self {
        self.gauge_type = value;
        self.kind = ChartKind::Gauge;
        self
    }
    /// 设置仪表盘指针宽度（非负）。
    pub fn pointer_width(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.pointer_width = value.max(0.0);
        }
        self
    }
    /// 设置仪表盘指针颜色。
    pub fn pointer_color(mut self, value: Color) -> Self {
        self.pointer_color = Some(value);
        self
    }
    /// 设置数值格式化函数（仪表盘读数）。
    pub fn format<F>(mut self, value: F) -> Self
    where
        F: Fn(f32) -> String + 'static,
    {
        self.value_format = Some(Rc::new(value));
        self
    }
    /// 设置图表背景色。
    pub fn bg(mut self, value: Color) -> Self {
        self.background = Some(value);
        self
    }
    /// 设置内边距（非负）。
    pub fn padding(mut self, value: f32) -> Self {
        if value.is_finite() {
            self.padding = value.max(0.0);
        }
        self
    }
    /// 配置入场动画并创建播放器。
    pub fn animation(mut self, value: AnimationConfig) -> Self {
        self.animation_config = Some(value);
        self.animation_player = Some(TransitionPlayer::new(value));
        self
    }
    /// 开启响应式尺寸（随父约束伸缩）。
    pub fn responsive(mut self, value: bool) -> Self {
        self.responsive = value;
        self
    }
    /// 配置交互（点击/平移/缩放/十字线）。
    pub fn interactive(mut self, value: InteractionConfig) -> Self {
        self.interaction = Some(value);
        self
    }
    /// 配置框选。
    pub fn brush(mut self, value: BrushConfig) -> Self {
        self.brush_config = Some(value);
        self
    }
    /// 配置 tooltip。
    pub fn tooltip(mut self, value: TooltipConfig) -> Self {
        self.tooltip_config = Some(value);
        self
    }
}
