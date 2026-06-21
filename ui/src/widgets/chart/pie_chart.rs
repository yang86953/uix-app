//! PieChart — pie / donut chart with proportional circular sectors.

use crate::define_widget;
use uix_graphics::Color;
use uix_core::{Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

#[derive(Debug, Clone)]
pub struct PieData {
    pub label: String,
    pub value: f32,
    pub color: Color,
}

impl PieData {
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self { label: label.into(), value, color }
    }
}

define_widget! {
    pub struct PieChart {
        data: Vec<PieData>,
        fixed_size: f32,
        hole_radius: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        let s = if self.fixed_size > 0.0 { self.fixed_size } else { 180.0 };
        Size::new(s, s)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let n = self.data.len();
        if n == 0 { return; }
        let total: f32 = self.data.iter().map(|d| d.value).sum();
        if total <= 0.0 { return; }

        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = (frame.w.min(frame.h) * 0.5 - 4.0).max(4.0);
        if r <= 0.0 { return; }

        let tokens = ctx.tokens();
        let text_c = tokens.color_text();
        let axis_c = tokens.color_border();
        let bg_c = tokens.color_bg_container();

        let mut sa = -std::f32::consts::FRAC_PI_2;

        for d in &self.data {
            let a = (d.value / total) * std::f32::consts::TAU;
            let ea = sa + a;
            let eng = ctx.engine();
            eng.fill_sector(cx, cy, r, sa, ea, d.color);
            eng.draw_line(cx, cy, cx + r * sa.cos(), cy + r * sa.sin(), text_c, 1.0);
            sa = ea;
        }
        {
            let eng = ctx.engine();
            eng.draw_line(cx, cy, cx + r * sa.cos(), cy + r * sa.sin(), text_c, 1.0);
        }
        if self.hole_radius > 0.0 {
            let hr = r * self.hole_radius;
            let eng = ctx.engine();
            eng.fill_circle(cx, cy, hr, bg_c);
            eng.stroke_circle(cx, cy, hr, axis_c, 1.0);
        }
    }
}

impl Default for PieChart { fn default() -> Self { Self::new() } }
impl PieChart {
    pub fn new() -> Self { Self { data: Vec::new(), fixed_size: 0.0, hole_radius: 0.0 } }
    pub fn data(mut self, d: Vec<PieData>) -> Self { self.data = d; self }
    pub fn size(mut self, s: f32) -> Self { self.fixed_size = s; self }
    pub fn donut(mut self, r: f32) -> Self { self.hole_radius = r.clamp(0.0, 0.9); self }
}
