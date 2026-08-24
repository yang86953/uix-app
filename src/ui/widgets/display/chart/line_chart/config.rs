//! 折线图的默认值、构造入口与链式配置。

// 引入运行时类型擦除，用于兼容现有多序列配置入口。
use std::any::Any;

// 复用父组件拥有的数据、主题和交互契约。
use super::*;

impl Default for LineChart {
    fn default() -> Self {
        Self::new()
    }
}

impl LineChart {
    /// 创建使用默认尺寸、网格和数据点标记的空折线图。
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            fixed_width: 0.0,
            fixed_height: LINE_CHART_VISUAL.defaults.height,
            line_color: None,
            max_value: 0.0,
            auto_min: false,
            show_grid: LINE_CHART_VISUAL.defaults.show_grid,
            show_dots: LINE_CHART_VISUAL.defaults.show_dots,
            line_width: LINE_CHART_VISUAL.defaults.line_width,
            dot_radius: LINE_CHART_VISUAL.defaults.dot_radius,
            series: Vec::new(),
            legend: LegendPosition::None,
            smooth: false,
            step: false,
            background: None,
            padding: LINE_CHART_VISUAL.defaults.padding,
            title: String::new(),
            subtitle: String::new(),
            responsive: false,
            interaction: None,
            brush_config: None,
            tooltip_config: None,
            animation_enabled: false,
            last_frame: Cell::new(None),
            hovered_pos: Cell::new(None),
            tooltip_pos: Cell::new(None),
            brush_start: Cell::new(None),
            pan_start: Cell::new(None),
            pan_origin: Cell::new(0.0),
            pan_offset: Cell::new(0.0),
            zoom: Cell::new(1.0),
            visual: LINE_CHART_VISUAL_REF,
            authored: LineChartAuthored::default(),
            points_scratch: RefCell::new(Vec::new()),
            smooth_points_scratch: RefCell::new(Vec::new()),
        }
    }

    /// 替换单序列折线图的数据点。
    pub fn data(mut self, data: Vec<LineData>) -> Self {
        self.data = data;
        self
    }

    /// 设置固定宽度；非有限或非正值恢复为自动宽度。
    pub fn width(mut self, width: f32) -> Self {
        self.fixed_width = Self::optional_dimension(width);
        self
    }

    /// 设置固定高度；非有限或非正值恢复为默认高度。
    pub fn height(mut self, height: f32) -> Self {
        let height = Self::optional_dimension(height);
        self.authored.set(LineChartAuthored::HEIGHT, height > 0.0);
        self.fixed_height = height;
        self
    }

    /// 设置单序列折线与数据点使用的颜色。
    pub fn line_color(mut self, color: Color) -> Self {
        self.line_color = Some(color);
        self
    }

    /// 设置纵轴显式最大值；非有限或非正值恢复为自动上界。
    pub fn max_value(mut self, value: f32) -> Self {
        self.max_value = if value.is_finite() && value > 0.0 {
            value
        } else {
            0.0
        };
        self
    }

    /// 设置纵轴下界是否跟随数据最小值。
    pub fn auto_min(mut self, value: bool) -> Self {
        self.auto_min = value;
        self
    }

    /// 设置是否绘制纵轴参考网格。
    pub fn show_grid(mut self, value: bool) -> Self {
        self.show_grid = value;
        self.authored.set(LineChartAuthored::SHOW_GRID, true);
        self
    }

    /// 设置是否在折线顶点绘制数据点标记。
    pub fn show_dots(mut self, value: bool) -> Self {
        self.show_dots = value;
        self.authored.set(LineChartAuthored::SHOW_DOTS, true);
        self
    }

    /// 设置折线宽度；非有限或非正值回退为一个像素。
    pub fn line_width(mut self, width: f32) -> Self {
        self.line_width = if width.is_finite() && width > 0.0 {
            width
        } else {
            1.0
        };
        self.authored.set(LineChartAuthored::LINE_WIDTH, true);
        self
    }

    /// 设置数据点半径；非有限值归零，负值夹取为零。
    pub fn dot_radius(mut self, radius: f32) -> Self {
        self.dot_radius = if radius.is_finite() {
            radius.max(0.0)
        } else {
            0.0
        };
        self.authored.set(LineChartAuthored::DOT_RADIUS, true);
        self
    }

    /// 设置多序列数据；当前兼容入口仅接受折线数据序列列表。
    pub fn series<T: 'static>(mut self, series: T) -> Self {
        if let Ok(series) =
            (Box::new(series) as Box<dyn Any>).downcast::<Vec<ChartSeries<Vec<LineData>>>>()
        {
            self.series = *series;
            if let Some(first) = self.series.first() {
                self.data = first.data.clone();
            }
        }
        self
    }

    /// 设置图例相对绘图区的保留位置。
    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position;
        self
    }

    /// 设置是否以平滑曲线连接数据点。
    pub fn smooth(mut self, value: bool) -> Self {
        self.smooth = value;
        self
    }

    /// 设置是否以阶梯折线连接数据点。
    pub fn step(mut self, value: bool) -> Self {
        self.step = value;
        self
    }

    /// 设置图表内容区域的背景颜色。
    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// 设置图表内容内边距；非有限值归零，负值夹取为零。
    pub fn padding(mut self, padding: f32) -> Self {
        let authored = padding.is_finite();
        self.padding = if authored {
            padding.max(0.0)
        } else {
            self.visual.defaults.padding
        };
        self.authored.set(LineChartAuthored::PADDING, authored);
        self
    }

    /// 设置显示在绘图区上方的标题。
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// 设置显示在标题下方的副标题。
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }

    /// 设置图表是否使用父约束提供的响应式尺寸。
    pub fn responsive(mut self, responsive: bool) -> Self {
        self.responsive = responsive;
        self
    }

    /// 启用并配置缩放、平移或数据点击交互。
    pub fn interactive(mut self, config: InteractionConfig) -> Self {
        self.interaction = Some(config);
        self
    }

    /// 启用并配置绘图区范围刷选交互。
    pub fn brush(mut self, config: BrushConfig) -> Self {
        self.brush_config = Some(config);
        self
    }

    /// 启用并配置数据提示框。
    pub fn tooltip(mut self, config: TooltipConfig) -> Self {
        self.tooltip_config = Some(config);
        self
    }

    /// 启用图表动画；当前保留配置参数供后续动画策略使用。
    pub fn animation(mut self, _animation: crate::ui::animation::AnimationConfig) -> Self {
        self.animation_enabled = true;
        self
    }
}
