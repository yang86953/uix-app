//! BarChart — vertical bar chart with auto-scaling and value labels.

use std::any::Any;
use std::cell::Cell;

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SnapshotFields, SystemEvent, WidgetTree};
use crate::widget;

use super::advanced::{
    BrushConfig, ChartSeries, InteractionConfig, LegendPosition, TooltipConfig, TooltipTrigger,
};

mod data;
mod plot;
mod presentation;

pub use self::data::BarData;
use self::plot::map_x;
use self::presentation::*;

// 使用一个字节记录会覆盖 UIX 默认值的 Rust 调用方声明。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct BarChartAuthored(u8);

impl BarChartAuthored {
    const HEIGHT: u8 = 1 << 0;
    const SHOW_VALUE: u8 = 1 << 1;
    const BAR_RADIUS: u8 = 1 << 2;
    const BAR_GAP: u8 = 1 << 3;
    const CATEGORY_GAP: u8 = 1 << 4;
    const PADDING: u8 = 1 << 5;

    fn contains(self, flag: u8) -> bool {
        self.0 & flag != 0
    }

    fn set(&mut self, flag: u8, authored: bool) {
        if authored {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }
}

widget! {
    /// 按具名数据项绘制柱形、坐标轴与可选数值标签的图表组件。
    pub struct BarChart {
        data: Vec<BarData>,
        fixed_width: f32,
        fixed_height: f32,
        max_value: f32,
        show_value: bool,
        bar_radius: f32,
        grouped: bool,
        stacked: bool,
        horizontal: bool,
        bar_gap: f32,
        category_gap: f32,
        series: Vec<ChartSeries<Vec<BarData>>>,
        legend: LegendPosition,
        background: Option<Color>,
        padding: f32,
        title: String,
        subtitle: String,
        responsive: bool,
        interaction: Option<InteractionConfig>,
        brush_config: Option<BrushConfig>,
        tooltip_config: Option<TooltipConfig>,
        animation_enabled: bool,
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
        #[snapshot(skip)]
        visual: &'static BarChartVisual,
        #[snapshot(skip)]
        authored: BarChartAuthored,
    }

    measure => (&self, constraints: Constraints) -> Size {
        let intrinsic = self.intrinsic_size();
        let width = super::responsive_extent(self.responsive, constraints.max.w, intrinsic.w);
        let height = super::responsive_extent(self.responsive, constraints.max.h, intrinsic.h);
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
                    self.pan_offset.set(
                        (self.pan_origin.get() + pos.x - start.x).clamp(-frame.w, frame.w),
                    );
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
                        if let Some(index) = self.data_index(*pos, frame) {
                            callback(self.data_label(index));
                            handled = true;
                        }
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

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        self.last_frame.set(Some(frame));
        let resolved = self.visual.resolve(ctx.tokens());
        let layout = self.visual.layout;
        let chrome = self.visual.chrome;
        let typography = self.visual.typography;
        ctx.fill_rect(
            frame,
            self.background.unwrap_or(resolved.background),
            None,
        );
        let mut content = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            ctx.draw_text(
                &self.title,
                Point::new(content.x, content.y),
                resolved.text,
                typography.title,
            );
            content.y += layout.title_height;
            content.h = (content.h - layout.title_height).max(0.0);
        }
        if !self.subtitle.is_empty() && content.h > 0.0 {
            ctx.draw_text(
                &self.subtitle,
                Point::new(content.x, content.y),
                resolved.text_secondary,
                typography.subtitle,
            );
            content.y += layout.subtitle_height;
            content.h = (content.h - layout.subtitle_height).max(0.0);
        }
        let legend_rect = self.reserve_legend(&mut content);
        ctx.push_clip(frame);
        let base_width = content.w;
        let zoom = self.zoom.get();
        if zoom > 1.0 {
            content.w = base_width * zoom;
            content.x -= (content.w - base_width) * 0.5;
        }
        content.x += self.pan_offset.get();
        let Some(plot) = self.plot_geometry(content) else {
            ctx.pop_clip();
            return;
        };

        let text_c = resolved.text;
        let label_c = resolved.text_secondary;
        let axis_c = resolved.border;

        if self.horizontal {
            let baseline_x = map_x(0.0, plot.chart_x, plot.chart_w, plot.min, plot.max);
            ctx.fill_rect(
                Rect::new(baseline_x, plot.chart_y, chrome.axis_stroke, plot.chart_h),
                axis_c,
                None,
            );
        } else {
            ctx.fill_rect(
                Rect::new(plot.chart_x, plot.baseline, plot.chart_w, chrome.axis_stroke),
                axis_c,
                None,
            );
        }

        let grid_lines = layout
            .grid_min_lines
            .max((plot.chart_h / layout.grid_min_spacing.max(f32::EPSILON)) as usize);
        for i in 0..=grid_lines {
            let t = i as f32 / grid_lines as f32;
            let value = plot.min + (plot.max - plot.min) * t;
            let label = Self::format_value(value);
            if self.horizontal {
                let gx = plot.chart_x + plot.chart_w * t;
                ctx.fill_rect(
                    Rect::new(gx, plot.chart_y, chrome.grid_stroke, plot.chart_h),
                    axis_c,
                    None,
                );
                let label_w = ctx.measure_text(label.as_str(), typography.grid).w;
                ctx.draw_text(
                    label.as_str(),
                    Point::new(
                        gx - label_w * layout.center_ratio,
                        plot.chart_y + plot.chart_h + chrome.value_gap,
                    ),
                    label_c,
                    typography.grid,
                );
            } else {
                let gy = plot.chart_y + plot.chart_h * (1.0 - t);
                ctx.fill_rect(
                    Rect::new(plot.chart_x, gy, plot.chart_w, chrome.grid_stroke),
                    axis_c,
                    None,
                );
                let y_label_rect = Rect::new(
                    frame.x,
                    gy - layout.category_label_height * layout.center_ratio,
                    (plot.y_label_w - chrome.value_gap).max(0.0),
                    layout.category_label_height,
                );
                let yly = ctx.visual_center_y(y_label_rect, typography.grid);
                let lsz = ctx.measure_text(label.as_str(), typography.grid);
                ctx.draw_text(
                    label.as_str(),
                    Point::new(plot.chart_x - lsz.w - chrome.label_gap, yly),
                    label_c,
                    typography.grid,
                );
            }
        }

        for &(series_index, item_index, rect) in &plot.items {
            let data = if self.series.is_empty() {
                self.data.as_slice()
            } else {
                self.series
                    .get(series_index)
                    .map_or(&[][..], |series| series.data.as_slice())
            };
            let Some(bar) = data.get(item_index) else {
                continue;
            };
            let radius = self
                .bar_radius
                .min(rect.w * 0.5)
                .min(rect.h * 0.5);
            let radius = (radius > 0.0).then(|| crate::draw::Radius::uniform(radius));
            if rect.w > 0.0 && rect.h > 0.0 {
                ctx.fill_rect(rect, bar.color, radius);
            }

            let value = Self::finite_value(bar.value);
            if self.show_value
                && if self.horizontal {
                    rect.w > layout.min_value_label_extent
                } else {
                    rect.h > layout.min_value_label_extent
                }
            {
                let s = Self::format_value(value);
                let sz = ctx.measure_text(s.as_str(), typography.value);
                if self.horizontal {
                    let value_x = if value >= 0.0 {
                        (rect.x + rect.w + chrome.value_gap).min(frame.x + frame.w - sz.w)
                    } else {
                        (rect.x - sz.w - chrome.value_gap).max(frame.x)
                    };
                    let value_rect = Rect::new(
                        value_x,
                        rect.y,
                        sz.w,
                        rect.h.max(sz.h + chrome.value_gap),
                    );
                    let value_y = ctx.visual_center_y(value_rect, typography.value);
                    ctx.draw_text(
                        s.as_str(),
                        Point::new(value_x, value_y),
                        text_c,
                        typography.value,
                    );
                } else {
                    let value_y = if value >= 0.0 {
                        (rect.y - sz.h - chrome.value_gap).max(frame.y)
                    } else {
                        (rect.y + rect.h + chrome.value_gap)
                            .min(frame.y + plot.chart_h - sz.h)
                    };
                    let val_rect = Rect::new(
                        rect.x,
                        value_y,
                        rect.w,
                        sz.h + chrome.value_gap,
                    );
                    let vy = ctx.visual_center_y(val_rect, typography.value);
                    ctx.draw_text(
                        s.as_str(),
                        Point::new(rect.x + (rect.w - sz.w) * layout.center_ratio, vy),
                        text_c,
                        typography.value,
                    );
                }
            }
            if series_index == 0 {
                let sz = ctx.measure_text(&bar.label, typography.category);
                if self.horizontal {
                    let lx = (plot.chart_x - sz.w - chrome.label_gap).max(frame.x);
                    let label_rect = Rect::new(
                        lx,
                        rect.y,
                        sz.w,
                        rect.h.max(layout.category_label_height),
                    );
                    let ly = ctx.visual_center_y(label_rect, typography.category);
                    ctx.draw_text(
                        &bar.label,
                        Point::new(lx, ly),
                        label_c,
                        typography.category,
                    );
                } else {
                    let max_label_x = (plot.chart_x + plot.chart_w - sz.w).max(plot.chart_x);
                    let lx = (rect.x + (rect.w - sz.w) * layout.center_ratio)
                        .clamp(plot.chart_x, max_label_x);
                    let label_rect = Rect::new(
                        lx,
                        content.y + plot.chart_h + chrome.value_gap,
                        sz.w,
                        layout.category_label_height,
                    );
                    let ly = ctx.visual_center_y(label_rect, typography.category);
                    ctx.draw_text(
                        &bar.label,
                        Point::new(lx, ly),
                        label_c,
                        typography.category,
                    );
                }
            }
        }

        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = self.hovered_pos.get().filter(|pos| frame.contains(*pos)) {
                let crosshair = resolved.primary;
                ctx.fill_rect(
                    Rect::new(
                        pos.x,
                        plot.chart_y,
                        chrome.crosshair_stroke,
                        plot.chart_h,
                    ),
                    crosshair,
                    None,
                );
                ctx.fill_rect(
                    Rect::new(
                        plot.chart_x,
                        pos.y,
                        plot.chart_w,
                        chrome.crosshair_stroke,
                    ),
                    crosshair,
                    None,
                );
            }
        }

        if let (Some(start), Some(end)) = (self.brush_start.get(), self.hovered_pos.get()) {
            let x = start.x.min(end.x).clamp(frame.x, frame.x + frame.w);
            let y = start.y.min(end.y).clamp(frame.y, frame.y + frame.h);
            let right = start.x.max(end.x).clamp(frame.x, frame.x + frame.w);
            let bottom = start.y.max(end.y).clamp(frame.y, frame.y + frame.h);
            ctx.fill_rect(
                Rect::new(x, y, (right - x).max(0.0), (bottom - y).max(0.0)),
                // 框选填充：token 主色 + 固定 alpha（替换原硬编码 22,119,255，随主题换肤）。
                resolved.primary.with_alpha(chrome.brush_alpha),
                None,
            );
        }
        if let Some(config) = &self.tooltip_config {
            let pos = match config.trigger_mode() {
                TooltipTrigger::Hover => self.hovered_pos.get(),
                TooltipTrigger::Click => self.tooltip_pos.get(),
            };
            if let Some(pos) = pos.filter(|pos| frame.contains(*pos)) {
                self.paint_tooltip(ctx, frame, pos, resolved);
            }
        }

        if let Some(legend_rect) = legend_rect {
            self.paint_legend(ctx, legend_rect, resolved);
        }
        ctx.pop_clip();
    }
}

// 把数据/交互状态与 UIX 静态视觉融合为单一 BarChart 根节点。
fn build_bar_chart_view(mut kernel: BarChart, visual: &'static BarChartVisual) -> ViewNode {
    if !kernel.authored.contains(BarChartAuthored::HEIGHT) {
        kernel.fixed_height = visual.defaults.height;
    }
    if !kernel.authored.contains(BarChartAuthored::SHOW_VALUE) {
        kernel.show_value = visual.defaults.show_value;
    }
    if !kernel.authored.contains(BarChartAuthored::BAR_RADIUS) {
        kernel.bar_radius = visual.defaults.bar_radius;
    }
    if !kernel.authored.contains(BarChartAuthored::BAR_GAP) {
        kernel.bar_gap = visual.defaults.bar_gap;
    }
    if !kernel.authored.contains(BarChartAuthored::CATEGORY_GAP) {
        kernel.category_gap = visual.defaults.category_gap;
    }
    if !kernel.authored.contains(BarChartAuthored::PADDING) {
        kernel.padding = visual.defaults.padding;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让声明式 View 构建统一进入同目录 UIX 根。
fn build_bar_chart_uix_root(kernel: BarChart) -> ViewNode {
    crate::uix!("src/ui/widgets/display/chart/bar_chart/bar_chart.uix")
}

impl View for BarChart {
    fn build(self) -> ViewNode {
        build_bar_chart_uix_root(self)
    }
}

impl Default for BarChart {
    fn default() -> Self {
        Self::new()
    }
}
impl BarChart {
    /// 创建空的垂直柱状图，并默认显示数值标签。
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            fixed_width: 0.0,
            fixed_height: BAR_CHART_VISUAL.defaults.height,
            max_value: 0.0,
            show_value: BAR_CHART_VISUAL.defaults.show_value,
            bar_radius: BAR_CHART_VISUAL.defaults.bar_radius,
            grouped: false,
            stacked: false,
            horizontal: false,
            bar_gap: BAR_CHART_VISUAL.defaults.bar_gap,
            category_gap: BAR_CHART_VISUAL.defaults.category_gap,
            series: Vec::new(),
            legend: LegendPosition::None,
            background: None,
            padding: BAR_CHART_VISUAL.defaults.padding,
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
            visual: BAR_CHART_VISUAL_REF,
            authored: BarChartAuthored::default(),
        }
    }
    /// 替换单序列柱状图的数据项。
    pub fn data(mut self, d: Vec<BarData>) -> Self {
        self.data = d;
        self
    }
    /// 设置固定宽度；非法或非正值恢复为自动宽度。
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = Self::optional_dimension(w);
        self
    }
    /// 设置固定高度；非法或非正值恢复为默认高度。
    pub fn height(mut self, h: f32) -> Self {
        let height = Self::optional_dimension(h);
        self.authored.set(BarChartAuthored::HEIGHT, height > 0.0);
        self.fixed_height = if height > 0.0 {
            height
        } else {
            self.visual.defaults.height
        };
        self
    }
    /// 设置纵轴显式最大值；非法或非正值恢复为自动上界。
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = if v.is_finite() && v > 0.0 { v } else { 0.0 };
        self
    }
    /// 设置是否在空间足够的柱条旁绘制数值标签。
    pub fn show_value(mut self, v: bool) -> Self {
        self.show_value = v;
        self.authored.set(BarChartAuthored::SHOW_VALUE, true);
        self
    }
    /// 设置柱条圆角半径；非有限值归零，负值夹取为零。
    pub fn bar_radius(mut self, r: f32) -> Self {
        self.bar_radius = if r.is_finite() { r.max(0.0) } else { 0.0 };
        self.authored.set(BarChartAuthored::BAR_RADIUS, true);
        self
    }

    /// 设置多序列是否在同一分类内并排分组显示。
    pub fn grouped(mut self, value: bool) -> Self {
        self.grouped = value;
        self
    }
    /// 设置多序列是否在同一分类内堆叠显示，并优先于分组模式。
    pub fn stacked(mut self, value: bool) -> Self {
        self.stacked = value;
        self
    }
    /// 设置柱条是否沿水平方向从纵轴伸展。
    pub fn horizontal(mut self, value: bool) -> Self {
        self.horizontal = value;
        self
    }
    /// 设置同一分类内柱条间距比例，并夹取到零至 0.9。
    pub fn bar_gap(mut self, value: f32) -> Self {
        let authored = value.is_finite();
        self.bar_gap = if authored {
            value.clamp(0.0, 0.9)
        } else {
            self.visual.defaults.bar_gap
        };
        self.authored.set(BarChartAuthored::BAR_GAP, authored);
        self
    }
    /// 设置相邻分类之间的留白比例，并夹取到零至 0.9。
    pub fn category_gap(mut self, value: f32) -> Self {
        let authored = value.is_finite();
        self.category_gap = if authored {
            value.clamp(0.0, 0.9)
        } else {
            self.visual.defaults.category_gap
        };
        self.authored.set(BarChartAuthored::CATEGORY_GAP, authored);
        self
    }
    /// 设置多序列数据；当前兼容入口仅接受柱状数据序列列表。
    pub fn series<T: 'static>(mut self, series: T) -> Self {
        if let Ok(series) =
            (Box::new(series) as Box<dyn Any>).downcast::<Vec<ChartSeries<Vec<BarData>>>>()
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
        self.authored.set(BarChartAuthored::PADDING, authored);
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
    /// 启用并配置柱条数据提示框。
    pub fn tooltip(mut self, config: TooltipConfig) -> Self {
        self.tooltip_config = Some(config);
        self
    }
    /// 启用图表动画；当前保留配置参数供后续动画策略使用。
    pub fn animation(mut self, _animation: crate::ui::animation::AnimationConfig) -> Self {
        self.animation_enabled = true;
        self
    }

    // 测试目标保留柱形几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Rect>)> {
        self.plot_geometry(frame)
            .map(|plot| (plot.baseline, plot.bars))
    }

    // 测试目标保留分组柱形几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn geometry_items_for_test(&self, frame: Rect) -> Option<Vec<(usize, usize, Rect)>> {
        self.plot_geometry(frame).map(|plot| plot.items)
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let last_frame = self.last_frame.get();
        let hovered_pos = self.hovered_pos.get();
        let tooltip_pos = self.tooltip_pos.get();
        let pan_offset = self.pan_offset.get();
        let zoom = self.zoom.get();
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
    }

    // 测试目标保留柱形 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    // 测试目标保留柱形交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留柱形 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::BarChart {
            data: self.data.clone(),
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            max_value: self.max_value,
            show_value: self.show_value,
            bar_radius: self.bar_radius,
        }
    }
}
