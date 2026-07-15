//! LineChart — line chart with grid lines and data point markers.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::{SnapshotFields, WidgetTree};

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
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let Some(plot) = self.plot_geometry(frame) else { return; };

        let tokens = ctx.tokens();
        let lc = self.line_color.unwrap_or(tokens.color_text());
        let lbc = tokens.color_text_secondary();
        let ac = tokens.color_border();
        let bg = tokens.color_bg_container();
        let _text_c = tokens.color_text();

        if self.show_grid {
            let grid_lines = 4.max((plot.chart_h / 30.0) as usize);
            for i in 0..=grid_lines {
                let t = i as f32 / grid_lines as f32;
                let gy = frame.y + plot.chart_h * (1.0 - t);
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

        let lw = self.line_width;
        let half = (lw * 0.5).floor() as i32;
        for segment in plot.points.windows(2) {
            for o in -half..=half {
                let o = o as f32;
                ctx.canvas_2d().draw_line(
                    segment[0].x,
                    segment[0].y + o,
                    segment[1].x,
                    segment[1].y + o,
                    lc,
                    1.0,
                );
            }
        }

        if self.show_dots && self.dot_radius > 0.0 {
            for pt in &plot.points {
                ctx.fill_circle(pt.x, pt.y, self.dot_radius, lc);
                ctx.fill_circle(pt.x, pt.y, (self.dot_radius - 1.5).max(0.5), bg);
            }
        }

        for (i, d) in self.data.iter().enumerate() {
            let x = plot.points[i].x;
            let sz = ctx.measure_text(&d.label, 10.0);
            let max_label_x = (plot.chart_x + plot.chart_w - sz.w).max(plot.chart_x);
            let lx = (x - sz.w * 0.5).clamp(plot.chart_x, max_label_x);
            let label_rect = Rect::new(lx, frame.y + plot.chart_h + 2.0, sz.w, 12.0);
            let ly = ctx.visual_center_y(label_rect, 10.0);
            ctx.draw_text(&d.label, Point::new(lx, ly), lbc, 10.0);
        }
    }
}

#[derive(Debug)]
struct LinePlot {
    min: f32,
    max: f32,
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

    fn format_value(value: f32) -> String {
        if value == value.trunc() {
            format!("{value:.0}")
        } else {
            format!("{value:.1}")
        }
    }

    fn value_range(&self) -> Option<(f32, f32)> {
        let mut values = self.data.iter().map(|item| Self::finite_value(item.value));
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
        let range = max - min;
        let map_y = |value: f32| {
            let clamped = value.clamp(min, max);
            frame.y + (max - clamped) / range * chart_h
        };
        let points = if self.data.len() == 1 {
            vec![Point::new(
                chart_x + chart_w * 0.5,
                map_y(Self::finite_value(self.data[0].value)),
            )]
        } else {
            let step = chart_w / (self.data.len() - 1) as f32;
            self.data
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    Point::new(
                        chart_x + index as f32 * step,
                        map_y(Self::finite_value(item.value)),
                    )
                })
                .collect()
        };
        Some(LinePlot {
            min,
            max,
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.data = next.data;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.line_color = next.line_color;
        self.max_value = next.max_value;
        self.auto_min = next.auto_min;
        self.show_grid = next.show_grid;
        self.show_dots = next.show_dots;
        self.line_width = next.line_width;
        self.dot_radius = next.dot_radius;
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
