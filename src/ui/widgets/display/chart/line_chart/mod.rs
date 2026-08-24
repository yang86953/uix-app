//! LineChart — line chart with grid lines and data point markers.

use std::cell::{Cell, RefCell};

use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::view::{View, ViewNode};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SnapshotFields, SystemEvent, WidgetTree};
use crate::widget;

use super::advanced::{
    BrushConfig, ChartSeries, InteractionConfig, LegendPosition, TooltipConfig, TooltipDatum,
    TooltipTrigger, catmull_rom_points_into, normalized_ratio,
};
use super::value_label::ChartValueLabel;

// 将构造、默认值与链式配置集中到同目录组件配置模块。
mod config;
mod presentation;
use presentation::*;

#[cfg(test)]
#[path = "../../../../../../tests/unit/ui/widgets/display/chart/line_chart/tests.rs"]
mod tests;

// 使用一个字节记录会覆盖 UIX 默认值的 Rust 调用方声明。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct LineChartAuthored(u8);

impl LineChartAuthored {
    const HEIGHT: u8 = 1 << 0;
    const SHOW_GRID: u8 = 1 << 1;
    const SHOW_DOTS: u8 = 1 << 2;
    const LINE_WIDTH: u8 = 1 << 3;
    const DOT_RADIUS: u8 = 1 << 4;
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

/// 折线图中的单个分类数据点。
#[derive(Debug, Clone, PartialEq)]
pub struct LineData {
    /// 显示在横轴与提示框中的分类标签。
    pub label: String,
    /// 数据点在纵轴上的原始数值。
    pub value: f32,
}

impl LineData {
    /// 使用分类标签和数值创建数据点。
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

widget! {
    /// 按有序数据点绘制折线、标记与坐标轴的图表组件。
    pub struct LineChart {
        data: Vec<LineData>,
        fixed_width: f32,
        fixed_height: f32,
        line_color: Option<Color>,
        max_value: f32,
        auto_min: bool,
        show_grid: bool,
        show_dots: bool,
        line_width: f32,
        dot_radius: f32,
        series: Vec<ChartSeries<Vec<LineData>>>,
        legend: LegendPosition,
        smooth: bool,
        step: bool,
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
        visual: &'static LineChartVisual,
        #[snapshot(skip)]
        authored: LineChartAuthored,
        #[snapshot(skip)]
        points_scratch: RefCell<Vec<Point>>,
        #[snapshot(skip)]
        smooth_points_scratch: RefCell<Vec<Point>>,
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

        let default_line_color = self.line_color.unwrap_or(resolved.text);
        let label_color = resolved.text_secondary;
        let axis_color = resolved.border;
        let point_inner_color = resolved.background;

        if self.show_grid {
            let grid_lines = layout
                .grid_min_lines
                .max((plot.chart_h / layout.grid_min_spacing.max(f32::EPSILON)) as usize);
            for i in 0..=grid_lines {
                let t = i as f32 / grid_lines as f32;
                let gy = plot.plot_y + plot.chart_h * (1.0 - t);
                ctx.fill_rect(
                    Rect::new(plot.chart_x, gy, plot.chart_w, chrome.grid_stroke),
                    axis_color,
                    None,
                );
                let value = plot.min + (plot.max - plot.min) * t;
                let label = Self::format_value(value);
                let y_label_rect = Rect::new(
                    frame.x,
                    gy - layout.category_label_height * layout.center_ratio,
                    (plot.y_label_w - layout.category_label_gap).max(0.0),
                    layout.category_label_height,
                );
                let yly = ctx.visual_center_y(y_label_rect, typography.grid);
                let lsz = ctx.measure_text(label.as_str(), typography.grid);
                ctx.draw_text(
                    label.as_str(),
                    Point::new(plot.chart_x - lsz.w - chrome.label_gap, yly),
                    label_color,
                    typography.grid,
                );
            }
        }

        ctx.fill_rect(
            Rect::new(
                plot.chart_x,
                plot.baseline,
                plot.chart_w,
                chrome.axis_stroke,
            ),
            axis_color,
            None,
        );

        let lw = self.line_width;
        let mut points = self.points_scratch.borrow_mut();
        let mut smooth_points = self.smooth_points_scratch.borrow_mut();
        for series_index in 0..self.series_count() {
            let Some(data) = self.series_at(series_index) else {
                continue;
            };
            self.fill_points_for_data(data, &plot, &mut points);
            let lc = self.line_color.unwrap_or(match series_index {
                0 => default_line_color,
                1 => resolved.primary,
                2 => resolved.success,
                3 => resolved.warning,
                _ => resolved.error,
            });
            let draw_segment = |ctx: &mut PaintContext, from: Point, to: Point| {
                ctx.draw_line(from.x, from.y, to.x, to.y, lc, lw);
            };
            if self.step {
                for segment in points.windows(2) {
                    let mid = Point::new(segment[1].x, segment[0].y);
                    draw_segment(ctx, segment[0], mid);
                    draw_segment(ctx, mid, segment[1]);
                }
            } else if self.smooth {
                catmull_rom_points_into(
                    &points,
                    self.visual.motion.smooth_subdivisions,
                    &mut smooth_points,
                );
                for segment in smooth_points.windows(2) {
                    draw_segment(ctx, segment[0], segment[1]);
                }
            } else {
                for segment in points.windows(2) {
                    draw_segment(ctx, segment[0], segment[1]);
                }
            }

            if self.show_dots && self.dot_radius > 0.0 {
                for pt in points.iter() {
                    ctx.fill_circle(pt.x, pt.y, self.dot_radius, lc);
                    ctx.fill_circle(
                        pt.x,
                        pt.y,
                        (self.dot_radius - chrome.dot_inner_inset)
                            .max(chrome.dot_inner_min_radius),
                        point_inner_color,
                    );
                }
            }
        }
        drop(smooth_points);
        drop(points);

        for (i, d) in self.data.iter().enumerate() {
            let x = Self::point_x(i, self.data.len(), &plot);
            let sz = ctx.measure_text(&d.label, typography.category);
            let max_label_x = (plot.chart_x + plot.chart_w - sz.w).max(plot.chart_x);
            let lx = (x - sz.w * layout.center_ratio).clamp(plot.chart_x, max_label_x);
            let label_rect = Rect::new(
                lx,
                plot.plot_y + plot.chart_h + layout.category_label_gap,
                sz.w,
                layout.category_label_height,
            );
            let ly = ctx.visual_center_y(label_rect, typography.category);
            ctx.draw_text(
                &d.label,
                Point::new(lx, ly),
                label_color,
                typography.category,
            );
        }

        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = self.hovered_pos.get().filter(|pos| frame.contains(*pos)) {
                ctx.fill_rect(
                    Rect::new(
                        pos.x,
                        plot.plot_y,
                        chrome.crosshair_stroke,
                        plot.chart_h,
                    ),
                    resolved.primary,
                    None,
                );
                ctx.fill_rect(
                    Rect::new(
                        plot.chart_x,
                        pos.y,
                        plot.chart_w,
                        chrome.crosshair_stroke,
                    ),
                    resolved.primary,
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

// 把数据/交互状态与 UIX 静态视觉融合为单一 LineChart 根节点。
fn build_line_chart_view(mut kernel: LineChart, declared_visual: LineChartVisual) -> ViewNode {
    let visual = UIX_LINE_CHART_VISUAL.get_or_init(|| declared_visual);
    debug_assert_eq!(*visual, declared_visual);
    if !kernel.authored.contains(LineChartAuthored::HEIGHT) {
        kernel.fixed_height = visual.defaults.height;
    }
    if !kernel.authored.contains(LineChartAuthored::SHOW_GRID) {
        kernel.show_grid = visual.defaults.show_grid;
    }
    if !kernel.authored.contains(LineChartAuthored::SHOW_DOTS) {
        kernel.show_dots = visual.defaults.show_dots;
    }
    if !kernel.authored.contains(LineChartAuthored::LINE_WIDTH) {
        kernel.line_width = visual.defaults.line_width;
    }
    if !kernel.authored.contains(LineChartAuthored::DOT_RADIUS) {
        kernel.dot_radius = visual.defaults.dot_radius;
    }
    if !kernel.authored.contains(LineChartAuthored::PADDING) {
        kernel.padding = visual.defaults.padding;
    }
    kernel.visual = visual;
    ViewNode::leaf(kernel)
}

// 让声明式 View 构建统一进入同目录 UIX 根。
fn build_line_chart_uix_root(kernel: LineChart) -> ViewNode {
    crate::uix!("src/ui/widgets/display/chart/line_chart/line_chart.uix")
}

impl View for LineChart {
    fn build(self) -> ViewNode {
        build_line_chart_uix_root(self)
    }
}

#[derive(Debug)]
struct LinePlot {
    min: f32,
    max: f32,
    plot_y: f32,
    chart_x: f32,
    chart_w: f32,
    chart_h: f32,
    y_label_w: f32,
    baseline: f32,
}

impl LineChart {
    fn intrinsic_size(&self) -> Size {
        let width = if self.fixed_width > 0.0 {
            self.fixed_width
        } else {
            self.visual.defaults.width
        };
        let height = if self.fixed_height > 0.0 {
            self.fixed_height
        } else {
            self.visual.defaults.height
        };
        Size::new(width, height)
    }

    fn optional_dimension(value: f32) -> f32 {
        if value.is_finite() && value > 0.0 {
            value
        } else {
            0.0
        }
    }

    fn finite_value(value: f32) -> f32 {
        if value.is_finite() { value } else { 0.0 }
    }

    fn reserve_legend(&self, content: &mut Rect) -> Option<Rect> {
        if self.legend == LegendPosition::None {
            return None;
        }
        let layout = self.visual.layout;
        match self.legend {
            LegendPosition::Top => {
                let height = layout.legend_row_height.min(content.h);
                let rect = Rect::new(content.x, content.y, content.w, height);
                content.y += height;
                content.h = (content.h - layout.legend_row_height).max(0.0);
                Some(rect)
            }
            LegendPosition::Bottom => {
                let height = layout.legend_row_height.min(content.h);
                let rect = Rect::new(content.x, content.y + content.h - height, content.w, height);
                content.h = (content.h - layout.legend_row_height).max(0.0);
                Some(rect)
            }
            LegendPosition::Left | LegendPosition::Right => {
                let width = (content.w * layout.legend_side_ratio)
                    .clamp(layout.legend_side_min, layout.legend_side_max)
                    .min(content.w);
                let x = if self.legend == LegendPosition::Left {
                    let x = content.x;
                    content.x += width;
                    x
                } else {
                    content.x + content.w - width
                };
                content.w = (content.w - width).max(0.0);
                Some(Rect::new(x, content.y, width, content.h))
            }
            LegendPosition::None => None,
        }
    }

    fn data_index(&self, pos: Point, frame: Rect) -> Option<usize> {
        let count = if self.series.is_empty() {
            self.data.len()
        } else {
            self.series.first().map_or(0, |series| series.data.len())
        };
        (count > 0 && frame.w > 0.0)
            .then(|| ((pos.x - frame.x) / frame.w * count as f32).floor() as usize)
            .map(|index| index.min(count - 1))
    }

    fn data_label(&self, index: usize) -> &str {
        if self.series.is_empty() {
            self.data.get(index).map_or("", |item| item.label.as_str())
        } else {
            self.series
                .first()
                .and_then(|series| series.data.get(index))
                .map_or("", |item| item.label.as_str())
        }
    }

    fn format_value(value: f32) -> ChartValueLabel {
        ChartValueLabel::from_f32(value)
    }

    fn paint_tooltip(
        &self,
        ctx: &mut PaintContext,
        frame: Rect,
        pos: Point,
        resolved: ResolvedLineChartVisual,
    ) {
        let Some(config) = &self.tooltip_config else {
            return;
        };
        let Some(datum) = self.tooltip_datum_at(pos, frame) else {
            return;
        };
        let text = config.format(&datum);
        let typography = self.visual.typography;
        let chrome = self.visual.chrome;
        let size = ctx.measure_text(&text, typography.tooltip);
        let x =
            (pos.x + chrome.tooltip_offset).min(frame.x + frame.w - size.w - chrome.tooltip_offset);
        let y = (pos.y - size.h - chrome.tooltip_offset).max(frame.y + chrome.tooltip_edge_inset);
        let rect = Rect::new(
            x.max(frame.x + chrome.tooltip_edge_inset),
            y,
            size.w + chrome.tooltip_padding * 2.0,
            size.h + chrome.tooltip_padding * 2.0,
        );
        ctx.fill_rect(rect, resolved.elevated, None);
        ctx.stroke_rect(rect, resolved.border, chrome.tooltip_border, None);
        ctx.draw_text(
            &text,
            Point::new(
                rect.x + chrome.tooltip_padding,
                rect.y + chrome.tooltip_padding,
            ),
            resolved.text,
            typography.tooltip,
        );
    }

    // 不拼接临时字符串，直接按测量结果居中绘制每个系列名称。
    fn paint_legend(&self, ctx: &mut PaintContext, frame: Rect, resolved: ResolvedLineChartVisual) {
        let typography = self.visual.typography;
        if self.series.is_empty() {
            ctx.text_center(
                typography.single_series_legend,
                frame,
                resolved.text_secondary,
                typography.legend,
            );
            return;
        }
        let gap = ctx
            .measure_text(typography.series_separator, typography.legend)
            .w;
        let text_width = self
            .series
            .iter()
            .map(|series| ctx.measure_text(&series.name, typography.legend).w)
            .sum::<f32>();
        let gaps = gap * self.series.len().saturating_sub(1) as f32;
        let mut x =
            frame.x + (frame.w - text_width - gaps).max(0.0) * self.visual.layout.center_ratio;
        let y = ctx.visual_center_y(frame, typography.legend);
        for series in &self.series {
            let width = ctx.measure_text(&series.name, typography.legend).w;
            ctx.draw_text(
                &series.name,
                Point::new(x, y),
                resolved.text_secondary,
                typography.legend,
            );
            x += width + gap;
        }
    }

    fn tooltip_datum_at(&self, pos: Point, frame: Rect) -> Option<TooltipDatum> {
        let index = self.data_index(pos, frame)?;
        let (series, item) = if self.series.is_empty() {
            (None, self.data.get(index)?)
        } else {
            let series = self.series.first()?;
            (Some(series.name.clone()), series.data.get(index)?)
        };
        Some(TooltipDatum {
            label: item.label.clone(),
            series,
            value: Some(Self::finite_value(item.value)),
            ..TooltipDatum::default()
        })
    }

    fn value_range(&self) -> Option<(f32, f32)> {
        let mut values = (0..self.series_count())
            .filter_map(|index| self.series_at(index))
            .flat_map(|items| items.iter())
            .map(|item| Self::finite_value(item.value));
        let first = values.next()?;
        let (data_min, data_max) = values.fold((first, first), |range, value| {
            (range.0.min(value), range.1.max(value))
        });
        let explicit_max = self.max_value > 0.0;
        let mut max = if explicit_max {
            self.max_value
        } else if self.auto_min {
            data_max
        } else {
            data_max.max(0.0)
        };
        let mut min = if self.auto_min {
            data_min.min(max)
        } else {
            0.0
        };
        if max <= min {
            let padding = min.abs().max(max.abs()).mul_add(0.1, 0.0).max(1.0);
            if self.auto_min {
                min -= padding;
                if !explicit_max {
                    max += padding;
                }
            } else {
                max = min + padding;
            }
        }
        Some((min, max))
    }

    fn series_count(&self) -> usize {
        if self.series.is_empty() {
            1
        } else {
            self.series.len()
        }
    }

    fn series_at(&self, index: usize) -> Option<&[LineData]> {
        if self.series.is_empty() {
            (index == 0).then_some(self.data.as_slice())
        } else {
            self.series.get(index).map(|series| series.data.as_slice())
        }
    }

    fn fill_points_for_data(&self, data: &[LineData], plot: &LinePlot, output: &mut Vec<Point>) {
        output.clear();
        if data.is_empty() {
            return;
        }
        output.reserve(data.len());
        let map_y = |value: f32| {
            plot.plot_y + plot.chart_h
                - normalized_ratio(Self::finite_value(value), plot.min, plot.max) * plot.chart_h
        };
        if data.len() == 1 {
            output.push(Point::new(
                plot.chart_x + plot.chart_w * 0.5,
                map_y(data[0].value),
            ));
        } else {
            let step = plot.chart_w / (data.len() - 1) as f32;
            output.extend(data.iter().enumerate().map(|(index, item)| {
                Point::new(plot.chart_x + index as f32 * step, map_y(item.value))
            }));
        }
    }

    fn point_x(index: usize, count: usize, plot: &LinePlot) -> f32 {
        if count <= 1 {
            plot.chart_x + plot.chart_w * 0.5
        } else {
            plot.chart_x + plot.chart_w * index as f32 / (count - 1) as f32
        }
    }

    fn plot_geometry(&self, frame: Rect) -> Option<LinePlot> {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let (min, max) = self.value_range()?;
        let layout = self.visual.layout;
        let y_label_w = layout
            .y_label_width
            .min(frame.w * layout.y_label_width_ratio);
        let chart_x = frame.x + y_label_w;
        let chart_w = frame.w - y_label_w;
        let chart_h = frame.h - layout.plot_bottom_reserve;
        if chart_w <= 0.0 || chart_h <= 0.0 {
            return None;
        }
        let map_y = |value: f32| frame.y + chart_h - normalized_ratio(value, min, max) * chart_h;
        Some(LinePlot {
            min,
            max,
            plot_y: frame.y,
            chart_x,
            chart_w,
            chart_h,
            y_label_w,
            baseline: map_y(0.0),
        })
    }

    // 测试目标保留折线几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Point>)> {
        self.plot_geometry(frame).map(|plot| {
            let mut points = Vec::new();
            self.fill_points_for_data(&self.data, &plot, &mut points);
            (plot.baseline, points)
        })
    }

    // 测试目标保留折线序列点观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn series_points_for_test(&self, frame: Rect) -> Option<Vec<Vec<Point>>> {
        self.plot_geometry(frame).map(|plot| {
            (0..self.series_count())
                .filter_map(|index| self.series_at(index))
                .map(|data| {
                    let mut points = Vec::new();
                    self.fill_points_for_data(data, &plot, &mut points);
                    points
                })
                .collect()
        })
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let last_frame = self.last_frame.get();
        let hovered_pos = self.hovered_pos.get();
        let tooltip_pos = self.tooltip_pos.get();
        let pan_offset = self.pan_offset.get();
        let zoom = self.zoom.get();
        let points_scratch = std::mem::take(self.points_scratch.get_mut());
        let smooth_points_scratch = std::mem::take(self.smooth_points_scratch.get_mut());
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
        *self.points_scratch.get_mut() = points_scratch;
        *self.smooth_points_scratch.get_mut() = smooth_points_scratch;
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

    // 测试目标保留折线 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    // 测试目标保留折线交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留折线 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::LineChart {
            data: self.data.clone(),
            fixed_width: self.fixed_width,
            fixed_height: self.fixed_height,
            line_color: self.line_color,
            max_value: self.max_value,
            auto_min: self.auto_min,
            show_grid: self.show_grid,
            show_dots: self.show_dots,
            line_width: self.line_width,
            dot_radius: self.dot_radius,
        }
    }
}
