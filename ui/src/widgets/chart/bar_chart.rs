//! BarChart — vertical bar chart with auto-scaling and value labels.

use crate::define_widget;
use uix_graphics::Color;
use uix_core::{Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

#[derive(Debug, Clone)]
pub struct BarData {
    pub label: String,
    pub value: f32,
    pub color: Color,
}

impl BarData {
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self { label: label.into(), value, color }
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

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let w = if self.fixed_width > 0.0 { self.fixed_width } else { 300.0 };
        let h = if self.fixed_height > 0.0 { self.fixed_height } else { 200.0 };
        Size::new(w, h)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let n = self.data.len();
        if n == 0 || frame.w <= 0.0 || frame.h <= 0.0 { return; }

        let max_val = if self.max_value > 0.0 { self.max_value }
            else { self.data.iter().map(|d| d.value).fold(0.0_f32, f32::max) };
        if max_val <= 0.0 { return; }

        let gap = 4.0;
        let label_h = 16.0;
        let value_h = if self.show_value { 14.0 } else { 0.0 };
        let chart_area_h = (frame.h - label_h - value_h - 4.0).max(1.0);
        let bar_w = ((frame.w - gap) / n as f32 - gap).max(4.0);

        let br = if self.bar_radius > 0.0 { Some(uix_graphics::Radius::uniform(self.bar_radius)) } else { None };
        let tokens = ctx.tokens();
        let text_c = tokens.color_text();
        let label_c = tokens.color_text_secondary();
        let axis_c = tokens.color_border();

        let baseline = frame.y + chart_area_h;
        ctx.fill_rect(Rect::new(frame.x, baseline, frame.w, 1.0), axis_c, None);

        for (i, bar) in self.data.iter().enumerate() {
            let bx = frame.x + gap + i as f32 * (bar_w + gap);
            let bh = (bar.value / max_val) * chart_area_h;
            let by = baseline - bh;
            ctx.fill_rect(Rect::new(bx, by, bar_w, bh), bar.color, br);

            if self.show_value && bh > 10.0 {
                let s = if bar.value == bar.value.trunc() { format!("{:.0}", bar.value) } else { format!("{:.1}", bar.value) };
                let sz = ctx.measure_text(&s, 10.0);
                ctx.draw_text(&s, uix_core::Point::new(bx + (bar_w - sz.w) * 0.5, by - sz.h - 2.0), text_c, 10.0);
            }
            let sz = ctx.measure_text(&bar.label, 10.0);
            ctx.draw_text(&bar.label, uix_core::Point::new(bx + (bar_w - sz.w) * 0.5, baseline + 4.0), label_c, 10.0);
        }
    }
}

impl Default for BarChart { fn default() -> Self { Self::new() } }
impl BarChart {
    pub fn new() -> Self {
        Self { data: Vec::new(), fixed_width: 0.0, fixed_height: 200.0, max_value: 0.0, show_value: true, bar_radius: 2.0 }
    }
    pub fn data(mut self, d: Vec<BarData>) -> Self { self.data = d; self }
    pub fn width(mut self, w: f32) -> Self { self.fixed_width = w; self }
    pub fn height(mut self, h: f32) -> Self { self.fixed_height = h; self }
    pub fn max_value(mut self, v: f32) -> Self { self.max_value = v; self }
    pub fn show_value(mut self, v: bool) -> Self { self.show_value = v; self }
    pub fn bar_radius(mut self, r: f32) -> Self { self.bar_radius = r; self }
}
