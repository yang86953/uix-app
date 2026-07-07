//! BarChart — vertical bar chart with auto-scaling and value labels.

use crate::core::{Point, Rect, Size};
use crate::define_widget;
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

define_widget! {
    pub struct BarChart {
        data: Vec<BarData>,
        fixed_width: f32,
        fixed_height: f32,
        max_value: f32,
        show_value: bool,
        bar_radius: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let w = if self.fixed_width > 0.0 { self.fixed_width } else { 300.0 };
        let h = if self.fixed_height > 0.0 { self.fixed_height } else { 200.0 };
        Size::new(w, h)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let n = self.data.len();
        if n == 0 || frame.w <= 0.0 || frame.h <= 0.0 { return; }

        let max_val = if self.max_value > 0.0 { self.max_value }
            else { self.data.iter().map(|d| d.value).fold(0.0_f32, f32::max) };
        if max_val <= 0.0 { return; }

        let tokens = ctx.tokens();
        let text_c = tokens.color_text();
        let label_c = tokens.color_text_secondary();
        let axis_c = tokens.color_border();

        let gap = 4.0;
        let y_label_w = 36.0;
        let label_h = 14.0;
        let value_h = if self.show_value { 14.0 } else { 0.0 };
        let chart_x = frame.x + y_label_w;
        let chart_w = (frame.w - y_label_w).max(1.0);
        let chart_area_h = (frame.h - label_h - value_h - 4.0).max(1.0);
        let bar_w = ((chart_w - gap) / n as f32 - gap).max(4.0);

        let br = if self.bar_radius > 0.0 { Some(crate::draw::Radius::uniform(self.bar_radius)) } else { None };
        let baseline = frame.y + chart_area_h;
        ctx.fill_rect(Rect::new(chart_x, baseline, chart_w, 1.0), axis_c, None);

        let gl = 4.max((chart_area_h / 30.0) as usize);
        for i in 0..gl {
            let t = (i as f32 + 1.0) / gl as f32;
            let gy = frame.y + chart_area_h * (1.0 - t);
            ctx.fill_rect(Rect::new(chart_x, gy, chart_w, 0.5), axis_c, None);
            let val = max_val * t;
            let label = if val == val.trunc() { format!("{:.0}", val) } else { format!("{:.1}", val) };
            let y_label_rect = Rect::new(frame.x, gy - 6.0, y_label_w - 2.0, 12.0);
            let yly = ctx.visual_center_y(y_label_rect, 9.0);
            let lsz = ctx.measure_text(&label, 9.0);
            ctx.draw_text(&label, Point::new(chart_x - lsz.w - 4.0, yly), label_c, 9.0);
        }

        for (i, bar) in self.data.iter().enumerate() {
            let bx = chart_x + gap + i as f32 * (bar_w + gap);
            let bh = (bar.value / max_val) * chart_area_h;
            let by = baseline - bh;
            ctx.fill_rect(Rect::new(bx, by, bar_w, bh), bar.color, br);

            if self.show_value && bh > 10.0 {
                let s = if bar.value == bar.value.trunc() { format!("{:.0}", bar.value) } else { format!("{:.1}", bar.value) };
                let sz = ctx.measure_text(&s, 10.0);
                let val_rect = Rect::new(bx, by - sz.h - 4.0, bar_w, sz.h + 2.0);
                let vy = ctx.visual_center_y(val_rect, 10.0);
                ctx.draw_text(&s, crate::core::Point::new(bx + (bar_w - sz.w) * 0.5, vy), text_c, 10.0);
            }
            let sz = ctx.measure_text(&bar.label, 10.0);
            let lx = bx + (bar_w - sz.w) * 0.5;
            let label_rect = Rect::new(lx, baseline + 2.0, sz.w, label_h - 2.0);
            let ly = ctx.visual_center_y(label_rect, 10.0);
            ctx.draw_text(&bar.label, crate::core::Point::new(lx, ly), label_c, 10.0);
        }
    }
}

impl Default for BarChart {
    fn default() -> Self {
        Self::new()
    }
}
impl BarChart {
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
        self.fixed_width = w;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = h;
        self
    }
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = v;
        self
    }
    pub fn show_value(mut self, v: bool) -> Self {
        self.show_value = v;
        self
    }
    pub fn bar_radius(mut self, r: f32) -> Self {
        self.bar_radius = r;
        self
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
