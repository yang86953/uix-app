//! LineChart — line chart with grid lines and data point markers.

use std::any::Any;
use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::{EventResult, MouseButton, SnapshotFields, SystemEvent, WidgetTree};

use super::advanced::{
    catmull_rom_points, normalized_ratio, BrushConfig, ChartSeries, InteractionConfig,
    LegendPosition, TooltipConfig, TooltipDatum, TooltipTrigger,
};

#[derive(Debug, Clone, PartialEq)]
pub struct LineData {
    pub label: String,
    pub value: f32,
}

impl LineData {
    pub fn new(label: impl Into<String>, value: f32) -> Self {
        Self {
            label: label.into(),
            value,
        }
    }
}

component! {
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

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        self.last_frame.set(Some(frame));
        ctx.fill_rect(
            frame,
            self.background.unwrap_or(ctx.tokens().color_bg_container()),
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
                ctx.tokens().color_text(),
                15.0,
            );
            content.y += 20.0;
            content.h = (content.h - 20.0).max(0.0);
        }
        if !self.subtitle.is_empty() && content.h > 0.0 {
            ctx.draw_text(
                &self.subtitle,
                Point::new(content.x, content.y),
                ctx.tokens().color_text_secondary(),
                11.0,
            );
            content.y += 16.0;
            content.h = (content.h - 16.0).max(0.0);
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

        let (default_line_color, palette_primary, palette_success, palette_warning, palette_error,
            lbc, ac, bg) = {
            let tokens = ctx.tokens();
            (
                self.line_color.unwrap_or(tokens.color_text()),
                tokens.color_primary(),
                tokens.color_success(),
                tokens.color_warning(),
                tokens.color_error(),
                tokens.color_text_secondary(),
                tokens.color_border(),
                tokens.color_bg_container(),
            )
        };

        if self.show_grid {
            let grid_lines = 4.max((plot.chart_h / 30.0) as usize);
            for i in 0..=grid_lines {
                let t = i as f32 / grid_lines as f32;
                let gy = plot.plot_y + plot.chart_h * (1.0 - t);
                ctx.fill_rect(Rect::new(plot.chart_x, gy, plot.chart_w, 0.5), ac, None);
                let value = plot.min + (plot.max - plot.min) * t;
                let label = Self::format_value(value);
                let y_label_rect = Rect::new(frame.x, gy - 6.0, plot.y_label_w - 2.0, 12.0);
                let yly = ctx.visual_center_y(y_label_rect, 9.0);
                let lsz = ctx.measure_text(&label, 9.0);
                ctx.draw_text(
                    &label,
                    Point::new(plot.chart_x - lsz.w - 4.0, yly),
                    lbc,
                    9.0,
                );
            }
        }

        ctx.fill_rect(
            Rect::new(plot.chart_x, plot.baseline, plot.chart_w, 1.0),
            ac,
            None,
        );

        let series = self.series_data();
        let lw = self.line_width;
        let half = (lw * 0.5).floor() as i32;
        for (series_index, data) in series.iter().enumerate() {
            let points = self.points_for_data(data, &plot);
            let lc = self.line_color.unwrap_or(match series_index {
                0 => default_line_color,
                1 => palette_primary,
                2 => palette_success,
                3 => palette_warning,
                _ => palette_error,
            });
            let draw_segment = |ctx: &mut PaintContext<'_>, from: Point, to: Point| {
                for o in -half..=half {
                    let o = o as f32;
                    ctx.canvas_2d()
                        .draw_line(from.x, from.y + o, to.x, to.y + o, lc, 1.0);
                }
            };
            if self.step {
                for segment in points.windows(2) {
                    let mid = Point::new(segment[1].x, segment[0].y);
                    draw_segment(ctx, segment[0], mid);
                    draw_segment(ctx, mid, segment[1]);
                }
            } else {
                let line_points = if self.smooth {
                    catmull_rom_points(&points, 8)
                } else {
                    points.clone()
                };
                for segment in line_points.windows(2) {
                    draw_segment(ctx, segment[0], segment[1]);
                }
            }

            if self.show_dots && self.dot_radius > 0.0 {
                for pt in &points {
                    ctx.fill_circle(pt.x, pt.y, self.dot_radius, lc);
                    ctx.fill_circle(pt.x, pt.y, (self.dot_radius - 1.5).max(0.5), bg);
                }
            }
        }

        for (i, d) in self.data.iter().enumerate() {
            let x = plot.points[i].x;
            let sz = ctx.measure_text(&d.label, 10.0);
            let max_label_x = (plot.chart_x + plot.chart_w - sz.w).max(plot.chart_x);
            let lx = (x - sz.w * 0.5).clamp(plot.chart_x, max_label_x);
            let label_rect = Rect::new(lx, plot.plot_y + plot.chart_h + 2.0, sz.w, 12.0);
            let ly = ctx.visual_center_y(label_rect, 10.0);
            ctx.draw_text(&d.label, Point::new(lx, ly), lbc, 10.0);
        }

        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = self.hovered_pos.get().filter(|pos| frame.contains(*pos)) {
                let crosshair = ctx.tokens().color_primary();
                ctx.fill_rect(
                    Rect::new(pos.x, plot.plot_y, 1.0, plot.chart_h),
                    crosshair,
                    None,
                );
                ctx.fill_rect(
                    Rect::new(plot.chart_x, pos.y, plot.chart_w, 1.0),
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
                Color::from_rgba(22, 119, 255, 48),
                None,
            );
        }
        if let Some(config) = &self.tooltip_config {
            let pos = match config.trigger_mode() {
                TooltipTrigger::Hover => self.hovered_pos.get(),
                TooltipTrigger::Click => self.tooltip_pos.get(),
            };
            if let Some(pos) = pos.filter(|pos| frame.contains(*pos)) {
                self.paint_tooltip(ctx, frame, pos);
            }
        }

        if let Some(legend_rect) = legend_rect {
            let legend = if self.series.is_empty() {
                "数据".to_owned()
            } else {
                self.series
                    .iter()
                    .map(|series| series.name.as_str())
                    .collect::<Vec<_>>()
                    .join("  ")
            };
            ctx.text_center(
                &legend,
                legend_rect,
                lbc,
                10.0,
            );
        }
        ctx.pop_clip();
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
    points: Vec<Point>,
}

impl Default for LineChart {
    fn default() -> Self {
        Self::new()
    }
}
impl LineChart {
    const DEFAULT_WIDTH: f32 = 300.0;
    const DEFAULT_HEIGHT: f32 = 200.0;

    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            fixed_width: 0.0,
            fixed_height: 200.0,
            line_color: None,
            max_value: 0.0,
            auto_min: false,
            show_grid: true,
            show_dots: true,
            line_width: 2.0,
            dot_radius: 3.0,
            series: Vec::new(),
            legend: LegendPosition::None,
            smooth: false,
            step: false,
            background: None,
            padding: 0.0,
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
        }
    }
    pub fn data(mut self, d: Vec<LineData>) -> Self {
        self.data = d;
        self
    }
    pub fn width(mut self, w: f32) -> Self {
        self.fixed_width = Self::optional_dimension(w);
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = Self::optional_dimension(h);
        self
    }
    pub fn line_color(mut self, c: Color) -> Self {
        self.line_color = Some(c);
        self
    }
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = if v.is_finite() && v > 0.0 { v } else { 0.0 };
        self
    }
    pub fn auto_min(mut self, v: bool) -> Self {
        self.auto_min = v;
        self
    }
    pub fn show_grid(mut self, v: bool) -> Self {
        self.show_grid = v;
        self
    }
    pub fn show_dots(mut self, v: bool) -> Self {
        self.show_dots = v;
        self
    }
    pub fn line_width(mut self, w: f32) -> Self {
        self.line_width = if w.is_finite() && w > 0.0 { w } else { 1.0 };
        self
    }
    pub fn dot_radius(mut self, radius: f32) -> Self {
        self.dot_radius = if radius.is_finite() {
            radius.max(0.0)
        } else {
            0.0
        };
        self
    }

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
    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position;
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
    pub fn bg(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }
    pub fn padding(mut self, padding: f32) -> Self {
        self.padding = if padding.is_finite() {
            padding.max(0.0)
        } else {
            0.0
        };
        self
    }
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
    pub fn subtitle(mut self, subtitle: impl Into<String>) -> Self {
        self.subtitle = subtitle.into();
        self
    }
    pub fn responsive(mut self, responsive: bool) -> Self {
        self.responsive = responsive;
        self
    }
    pub fn interactive(mut self, config: InteractionConfig) -> Self {
        self.interaction = Some(config);
        self
    }
    pub fn brush(mut self, config: BrushConfig) -> Self {
        self.brush_config = Some(config);
        self
    }
    pub fn tooltip(mut self, config: TooltipConfig) -> Self {
        self.tooltip_config = Some(config);
        self
    }
    pub fn animation(mut self, _animation: crate::ui::animation::AnimationConfig) -> Self {
        self.animation_enabled = true;
        self
    }

    fn intrinsic_size(&self) -> Size {
        let width = if self.fixed_width > 0.0 {
            self.fixed_width
        } else {
            Self::DEFAULT_WIDTH
        };
        let height = if self.fixed_height > 0.0 {
            self.fixed_height
        } else {
            Self::DEFAULT_HEIGHT
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
        if value.is_finite() {
            value
        } else {
            0.0
        }
    }

    fn reserve_legend(&self, content: &mut Rect) -> Option<Rect> {
        if self.legend == LegendPosition::None {
            return None;
        }
        const ROW: f32 = 18.0;
        match self.legend {
            LegendPosition::Top => {
                let rect = Rect::new(content.x, content.y, content.w, ROW.min(content.h));
                content.y += ROW.min(content.h);
                content.h = (content.h - ROW).max(0.0);
                Some(rect)
            }
            LegendPosition::Bottom => {
                let height = ROW.min(content.h);
                let rect = Rect::new(content.x, content.y + content.h - height, content.w, height);
                content.h = (content.h - ROW).max(0.0);
                Some(rect)
            }
            LegendPosition::Left | LegendPosition::Right => {
                let width = (content.w * 0.24).clamp(64.0, 120.0).min(content.w);
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

    fn format_value(value: f32) -> String {
        if value == value.trunc() {
            format!("{value:.0}")
        } else {
            format!("{value:.1}")
        }
    }

    fn paint_tooltip(&self, ctx: &mut PaintContext<'_>, frame: Rect, pos: Point) {
        let Some(config) = &self.tooltip_config else {
            return;
        };
        let Some(datum) = self.tooltip_datum_at(pos, frame) else {
            return;
        };
        let text = config.format(&datum);
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
        let series = self.series_data();
        let mut values = series
            .iter()
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

    fn series_data(&self) -> Vec<&[LineData]> {
        if self.series.is_empty() {
            vec![self.data.as_slice()]
        } else {
            self.series
                .iter()
                .map(|series| series.data.as_slice())
                .collect()
        }
    }

    fn points_for_data(&self, data: &[LineData], plot: &LinePlot) -> Vec<Point> {
        if data.is_empty() {
            return Vec::new();
        }
        let map_y = |value: f32| {
            plot.plot_y + plot.chart_h
                - normalized_ratio(Self::finite_value(value), plot.min, plot.max) * plot.chart_h
        };
        if data.len() == 1 {
            vec![Point::new(
                plot.chart_x + plot.chart_w * 0.5,
                map_y(data[0].value),
            )]
        } else {
            let step = plot.chart_w / (data.len() - 1) as f32;
            data.iter()
                .enumerate()
                .map(|(index, item)| {
                    Point::new(plot.chart_x + index as f32 * step, map_y(item.value))
                })
                .collect()
        }
    }

    fn plot_geometry(&self, frame: Rect) -> Option<LinePlot> {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let (min, max) = self.value_range()?;
        let y_label_w = 36.0_f32.min(frame.w * 0.35);
        let chart_x = frame.x + y_label_w;
        let chart_w = frame.w - y_label_w;
        let chart_h = frame.h - 14.0;
        if chart_w <= 0.0 || chart_h <= 0.0 {
            return None;
        }
        let map_y = |value: f32| frame.y + chart_h - normalized_ratio(value, min, max) * chart_h;
        let points = self.points_for_data(
            &self.data,
            &LinePlot {
                min,
                max,
                plot_y: frame.y,
                chart_x,
                chart_w,
                chart_h,
                y_label_w,
                baseline: map_y(0.0),
                points: Vec::new(),
            },
        );
        Some(LinePlot {
            min,
            max,
            plot_y: frame.y,
            chart_x,
            chart_w,
            chart_h,
            y_label_w,
            baseline: map_y(0.0),
            points,
        })
    }

    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Point>)> {
        self.plot_geometry(frame)
            .map(|plot| (plot.baseline, plot.points))
    }

    #[cfg(test)]
    pub(crate) fn series_points_for_test(&self, frame: Rect) -> Option<Vec<Vec<Point>>> {
        self.plot_geometry(frame).map(|plot| {
            self.series_data()
                .into_iter()
                .map(|data| self.points_for_data(data, &plot))
                .collect()
        })
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

    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

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
