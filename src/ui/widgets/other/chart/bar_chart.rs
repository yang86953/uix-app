//! BarChart — vertical bar chart with auto-scaling and value labels.

use std::any::Any;
use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::Color;
use crate::ui::core::paint_context::PaintContext;
use crate::ui::{EventResult, MouseButton, SnapshotFields, SystemEvent, WidgetTree};

use super::advanced::{
    normalized_ratio, BrushConfig, ChartSeries, InteractionConfig, LegendPosition, TooltipConfig,
    TooltipDatum, TooltipTrigger,
};

#[derive(Debug, Clone, PartialEq)]
pub struct BarData {
    pub label: String,
    pub value: f32,
    pub color: Color,
}

impl BarData {
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}

component! {
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

        let tokens = ctx.tokens();
        let text_c = tokens.color_text();
        let label_c = tokens.color_text_secondary();
        let axis_c = tokens.color_border();

        if self.horizontal {
            let baseline_x = map_x(0.0, plot.chart_x, plot.chart_w, plot.min, plot.max);
            ctx.fill_rect(
                Rect::new(baseline_x, plot.chart_y, 1.0, plot.chart_h),
                axis_c,
                None,
            );
        } else {
            ctx.fill_rect(
                Rect::new(plot.chart_x, plot.baseline, plot.chart_w, 1.0),
                axis_c,
                None,
            );
        }

        let grid_lines = 4.max((plot.chart_h / 30.0) as usize);
        for i in 0..=grid_lines {
            let t = i as f32 / grid_lines as f32;
            let value = plot.min + (plot.max - plot.min) * t;
            let label = Self::format_value(value);
            if self.horizontal {
                let gx = plot.chart_x + plot.chart_w * t;
                ctx.fill_rect(Rect::new(gx, plot.chart_y, 0.5, plot.chart_h), axis_c, None);
                let label_w = ctx.measure_text(&label, 9.0).w;
                ctx.draw_text(
                    &label,
                    Point::new(gx - label_w * 0.5, plot.chart_y + plot.chart_h + 2.0),
                    label_c,
                    9.0,
                );
            } else {
                let gy = plot.chart_y + plot.chart_h * (1.0 - t);
                ctx.fill_rect(Rect::new(plot.chart_x, gy, plot.chart_w, 0.5), axis_c, None);
                let y_label_rect = Rect::new(frame.x, gy - 6.0, plot.y_label_w - 2.0, 12.0);
                let yly = ctx.visual_center_y(y_label_rect, 9.0);
                let lsz = ctx.measure_text(&label, 9.0);
                ctx.draw_text(
                    &label,
                    Point::new(plot.chart_x - lsz.w - 4.0, yly),
                    label_c,
                    9.0,
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
            if self.show_value && if self.horizontal { rect.w > 10.0 } else { rect.h > 10.0 } {
                let s = Self::format_value(value);
                let sz = ctx.measure_text(&s, 10.0);
                if self.horizontal {
                    let value_x = if value >= 0.0 {
                        (rect.x + rect.w + 2.0).min(frame.x + frame.w - sz.w)
                    } else {
                        (rect.x - sz.w - 2.0).max(frame.x)
                    };
                    let value_rect = Rect::new(value_x, rect.y, sz.w, rect.h.max(sz.h + 2.0));
                    let value_y = ctx.visual_center_y(value_rect, 10.0);
                    ctx.draw_text(&s, Point::new(value_x, value_y), text_c, 10.0);
                } else {
                    let value_y = if value >= 0.0 {
                        (rect.y - sz.h - 2.0).max(frame.y)
                    } else {
                        (rect.y + rect.h + 2.0).min(frame.y + plot.chart_h - sz.h)
                    };
                    let val_rect = Rect::new(rect.x, value_y, rect.w, sz.h + 2.0);
                    let vy = ctx.visual_center_y(val_rect, 10.0);
                    ctx.draw_text(
                        &s,
                        Point::new(rect.x + (rect.w - sz.w) * 0.5, vy),
                        text_c,
                        10.0,
                    );
                }
            }
            if series_index == 0 {
                let sz = ctx.measure_text(&bar.label, 10.0);
                if self.horizontal {
                    let lx = (plot.chart_x - sz.w - 4.0).max(frame.x);
                    let label_rect = Rect::new(lx, rect.y, sz.w, rect.h.max(12.0));
                    let ly = ctx.visual_center_y(label_rect, 10.0);
                    ctx.draw_text(&bar.label, Point::new(lx, ly), label_c, 10.0);
                } else {
                    let max_label_x = (plot.chart_x + plot.chart_w - sz.w).max(plot.chart_x);
                    let lx = (rect.x + (rect.w - sz.w) * 0.5).clamp(plot.chart_x, max_label_x);
                    let label_rect = Rect::new(lx, content.y + plot.chart_h + 2.0, sz.w, 12.0);
                    let ly = ctx.visual_center_y(label_rect, 10.0);
                    ctx.draw_text(&bar.label, Point::new(lx, ly), label_c, 10.0);
                }
            }
        }

        if self
            .interaction
            .as_ref()
            .is_some_and(|config| config.crosshair)
        {
            if let Some(pos) = self.hovered_pos.get().filter(|pos| frame.contains(*pos)) {
                let crosshair = ctx.tokens().color_primary();
                ctx.fill_rect(
                    Rect::new(pos.x, plot.chart_y, 1.0, plot.chart_h),
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
            ctx.text_center(&legend, legend_rect, label_c, 10.0);
        }
        ctx.pop_clip();
    }
}

fn map_x(value: f32, x: f32, width: f32, min: f32, max: f32) -> f32 {
    x + normalized_ratio(value, min, max) * width
}

#[derive(Debug)]
struct BarPlot {
    min: f32,
    max: f32,
    chart_x: f32,
    chart_y: f32,
    chart_w: f32,
    chart_h: f32,
    y_label_w: f32,
    baseline: f32,
    #[cfg(test)]
    bars: Vec<Rect>,
    /// `(series_index, item_index, rect)` for grouped/stacked rendering.
    items: Vec<(usize, usize, Rect)>,
}

impl Default for BarChart {
    fn default() -> Self {
        Self::new()
    }
}
impl BarChart {
    const DEFAULT_WIDTH: f32 = 300.0;
    const DEFAULT_HEIGHT: f32 = 200.0;

    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            fixed_width: 0.0,
            fixed_height: 200.0,
            max_value: 0.0,
            show_value: true,
            bar_radius: 2.0,
            grouped: false,
            stacked: false,
            horizontal: false,
            bar_gap: 0.2,
            category_gap: 0.2,
            series: Vec::new(),
            legend: LegendPosition::None,
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
    pub fn data(mut self, d: Vec<BarData>) -> Self {
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
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = if v.is_finite() && v > 0.0 { v } else { 0.0 };
        self
    }
    pub fn show_value(mut self, v: bool) -> Self {
        self.show_value = v;
        self
    }
    pub fn bar_radius(mut self, r: f32) -> Self {
        self.bar_radius = if r.is_finite() { r.max(0.0) } else { 0.0 };
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
            0.2
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
    pub fn legend(mut self, position: LegendPosition) -> Self {
        self.legend = position;
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
        let (offset, extent) = if self.horizontal {
            (pos.y - frame.y, frame.h)
        } else {
            (pos.x - frame.x, frame.w)
        };
        (count > 0 && extent > 0.0)
            .then(|| (offset / extent * count as f32).floor() as usize)
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

    fn paint_tooltip(&self, ctx: &mut PaintContext, frame: Rect, pos: Point) {
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
        if series.is_empty() || series.iter().all(|items| items.is_empty()) {
            return None;
        }
        let mut min = 0.0_f32;
        let mut observed_max = 0.0_f32;
        if self.stacked && series.len() > 1 {
            let category_count = series.iter().map(|items| items.len()).max().unwrap_or(0);
            for index in 0..category_count {
                let (positive, negative) = series.iter().fold((0.0, 0.0), |(pos, neg), items| {
                    let value = items
                        .get(index)
                        .map_or(0.0, |item| Self::finite_value(item.value));
                    if value >= 0.0 {
                        (pos + value, neg)
                    } else {
                        (pos, neg + value)
                    }
                });
                min = min.min(negative);
                observed_max = observed_max.max(positive);
            }
        } else {
            for items in &series {
                for item in items.iter() {
                    let value = Self::finite_value(item.value);
                    min = min.min(value);
                    observed_max = observed_max.max(value);
                }
            }
        }
        let max = if self.max_value > 0.0 {
            self.max_value
        } else {
            observed_max
        };
        (max > min).then_some((min, max))
    }

    fn series_data(&self) -> Vec<&[BarData]> {
        if self.series.is_empty() {
            vec![self.data.as_slice()]
        } else {
            self.series
                .iter()
                .map(|series| series.data.as_slice())
                .collect()
        }
    }

    fn plot_geometry(&self, frame: Rect) -> Option<BarPlot> {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return None;
        }
        let (min, max) = self.value_range()?;
        let y_label_w = 36.0_f32.min(frame.w * 0.35);
        let chart_x = frame.x + y_label_w;
        let chart_w = frame.w - y_label_w;
        let label_h = 14.0;
        let value_h = if self.show_value { 14.0 } else { 0.0 };
        let chart_h = frame.h - label_h - value_h - 4.0;
        if chart_w <= 0.0 || chart_h <= 0.0 {
            return None;
        }
        let map_y = |value: f32| frame.y + chart_h - normalized_ratio(value, min, max) * chart_h;
        let baseline = map_y(0.0);
        let series = self.series_data();
        let category_count = series.iter().map(|items| items.len()).max().unwrap_or(0);
        if category_count == 0 {
            return None;
        }
        let category_extent = if self.horizontal { chart_h } else { chart_w };
        let category_slot = category_extent / category_count as f32;
        let series_count = series.len().max(1);
        let category_gap = category_slot * self.category_gap;
        let available = (category_slot - category_gap).max(0.0);
        #[cfg(test)]
        let mut bars = Vec::new();
        let mut items = Vec::new();
        for category in 0..category_count {
            if self.stacked && series_count > 1 {
                let mut positive = 0.0;
                let mut negative = 0.0;
                for (series_index, data) in series.iter().enumerate() {
                    let Some(bar) = data.get(category) else {
                        continue;
                    };
                    let value = Self::finite_value(bar.value);
                    let (start, end) = if value >= 0.0 {
                        let start = positive;
                        positive += value;
                        (start, positive)
                    } else {
                        let start = negative;
                        negative += value;
                        (start, negative)
                    };
                    let rect = if self.horizontal {
                        let x1 = map_x(start, chart_x, chart_w, min, max);
                        let x2 = map_x(end, chart_x, chart_w, min, max);
                        Rect::new(
                            x1.min(x2),
                            frame.y + category as f32 * category_slot + category_gap * 0.5,
                            (x2 - x1).abs(),
                            available,
                        )
                    } else {
                        let y1 = map_y(start);
                        let y2 = map_y(end);
                        Rect::new(
                            chart_x + category as f32 * category_slot + category_gap * 0.5,
                            y1.min(y2),
                            available,
                            (y2 - y1).abs(),
                        )
                    };
                    items.push((series_index, category, rect));
                    #[cfg(test)]
                    if series_index == 0 {
                        bars.push(rect);
                    }
                }
            } else {
                let visible_series = if self.grouped { series_count } else { 1 };
                let series_slot = available / visible_series as f32;
                let bar_extent = if visible_series > 1 {
                    series_slot * (1.0 - self.bar_gap)
                } else {
                    series_slot
                };
                let bar_inset = (series_slot - bar_extent) * 0.5;
                for series_index in 0..visible_series {
                    let Some(data) = series.get(series_index) else {
                        continue;
                    };
                    let Some(bar) = data.get(category) else {
                        continue;
                    };
                    let value = Self::finite_value(bar.value);
                    let rect = if self.horizontal {
                        let x1 = map_x(0.0, chart_x, chart_w, min, max);
                        let x2 = map_x(value, chart_x, chart_w, min, max);
                        Rect::new(
                            x1.min(x2),
                            frame.y
                                + category as f32 * category_slot
                                + category_gap * 0.5
                                + series_index as f32 * series_slot
                                + bar_inset,
                            (x2 - x1).abs(),
                            bar_extent,
                        )
                    } else {
                        let value_y = map_y(value);
                        let x = chart_x
                            + category as f32 * category_slot
                            + category_gap * 0.5
                            + series_index as f32 * series_slot
                            + bar_inset;
                        Rect::new(
                            x,
                            value_y.min(baseline),
                            bar_extent,
                            (value_y - baseline).abs(),
                        )
                    };
                    items.push((series_index, category, rect));
                    #[cfg(test)]
                    if series_index == 0 {
                        bars.push(rect);
                    }
                    if !self.grouped {
                        break;
                    }
                }
            }
        }
        Some(BarPlot {
            min,
            max,
            chart_x,
            chart_y: frame.y,
            chart_w,
            chart_h,
            y_label_w,
            baseline,
            #[cfg(test)]
            bars,
            items,
        })
    }

    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Rect>)> {
        self.plot_geometry(frame)
            .map(|plot| (plot.baseline, plot.bars))
    }

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
