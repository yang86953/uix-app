//! BarChart — vertical bar chart with auto-scaling and value labels.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::{SnapshotFields, WidgetTree};

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
        let text_c = tokens.color_text();
        let label_c = tokens.color_text_secondary();
        let axis_c = tokens.color_border();

        ctx.fill_rect(
            Rect::new(plot.chart_x, plot.baseline, plot.chart_w, 1.0),
            axis_c,
            None,
        );

        let grid_lines = 4.max((plot.chart_h / 30.0) as usize);
        for i in 0..=grid_lines {
            let t = i as f32 / grid_lines as f32;
            let gy = frame.y + plot.chart_h * (1.0 - t);
            ctx.fill_rect(Rect::new(plot.chart_x, gy, plot.chart_w, 0.5), axis_c, None);
            let value = plot.min + (plot.max - plot.min) * t;
            let label = Self::format_value(value);
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

        for (bar, rect) in self.data.iter().zip(&plot.bars) {
            let radius = self
                .bar_radius
                .min(rect.w * 0.5)
                .min(rect.h * 0.5);
            let radius = (radius > 0.0).then(|| crate::draw::Radius::uniform(radius));
            if rect.h > 0.0 {
                ctx.fill_rect(*rect, bar.color, radius);
            }

            let value = Self::finite_value(bar.value);
            if self.show_value && rect.h > 10.0 {
                let s = Self::format_value(value);
                let sz = ctx.measure_text(&s, 10.0);
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
            let sz = ctx.measure_text(&bar.label, 10.0);
            let max_label_x = (plot.chart_x + plot.chart_w - sz.w).max(plot.chart_x);
            let lx = (rect.x + (rect.w - sz.w) * 0.5).clamp(plot.chart_x, max_label_x);
            let label_rect = Rect::new(lx, frame.y + plot.chart_h + 2.0, sz.w, 12.0);
            let ly = ctx.visual_center_y(label_rect, 10.0);
            ctx.draw_text(&bar.label, Point::new(lx, ly), label_c, 10.0);
        }
    }
}

#[derive(Debug)]
struct BarPlot {
    min: f32,
    max: f32,
    chart_x: f32,
    chart_w: f32,
    chart_h: f32,
    y_label_w: f32,
    baseline: f32,
    bars: Vec<Rect>,
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
        if self.data.is_empty() {
            return None;
        }
        let (min, observed_max) = self.data.iter().fold((0.0_f32, 0.0_f32), |range, item| {
            let value = Self::finite_value(item.value);
            (range.0.min(value), range.1.max(value))
        });
        let max = if self.max_value > 0.0 {
            self.max_value
        } else {
            observed_max
        };
        (max > min).then_some((min, max))
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
        let range = max - min;
        let map_y = |value: f32| {
            let clamped = value.clamp(min, max);
            frame.y + (max - clamped) / range * chart_h
        };
        let baseline = map_y(0.0);
        let group_w = chart_w / self.data.len() as f32;
        let gap = (group_w * 0.2).clamp(1.0, 4.0);
        let bar_w = (group_w - gap).max(1.0).min(group_w);
        let bars = self
            .data
            .iter()
            .enumerate()
            .map(|(index, bar)| {
                let value_y = map_y(Self::finite_value(bar.value));
                let x = chart_x + index as f32 * group_w + (group_w - bar_w) * 0.5;
                Rect::new(x, value_y.min(baseline), bar_w, (value_y - baseline).abs())
            })
            .collect();
        Some(BarPlot {
            min,
            max,
            chart_x,
            chart_w,
            chart_h,
            y_label_w,
            baseline,
            bars,
        })
    }

    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Rect>)> {
        self.plot_geometry(frame)
            .map(|plot| (plot.baseline, plot.bars))
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.data = next.data;
        self.fixed_width = next.fixed_width;
        self.fixed_height = next.fixed_height;
        self.max_value = next.max_value;
        self.show_value = next.show_value;
        self.bar_radius = next.bar_radius;
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
