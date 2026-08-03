use crate::core::{Point, Rect};
use crate::draw::{Color, FillRule, PathBuilder};
use crate::ui::component::paint_context::PaintContext;

use super::super::bar_chart::BarData;
use super::super::line_chart::LineData;
use super::{
    ChartKind, ChartPayload, ChartPlaceholder, LineStyle, PointStyle, RadarShape,
    TooltipTrigger, catmull_rom_points, finite_or_zero, normalized_ratio, palette_color,
};

impl ChartPlaceholder {

    pub(crate) fn paint(&self, ctx: &mut PaintContext, frame: Rect) {
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

    pub(crate) fn paint_interaction(&self, ctx: &mut PaintContext, frame: Rect) {
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

    pub(crate) fn paint_axes(&self, ctx: &mut PaintContext, plot: Rect) {
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

    pub(crate) fn paint_bars(&self, ctx: &mut PaintContext, plot: Rect) {
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

    pub(crate) fn paint_lines(&self, ctx: &mut PaintContext, plot: Rect) {
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

    pub(crate) fn paint_scatter(&self, ctx: &mut PaintContext, plot: Rect) {
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

    pub(crate) fn paint_radar(&self, ctx: &mut PaintContext, plot: Rect) {
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


}

