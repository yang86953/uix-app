//! PieChart — pie / donut chart with proportional circular sectors.

use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::{SnapshotFields, WidgetTree};

#[derive(Debug, Clone, PartialEq)]
pub struct PieData {
    pub label: String,
    pub value: f32,
    pub color: Color,
}

impl PieData {
    pub fn new(label: impl Into<String>, value: f32, color: Color) -> Self {
        Self {
            label: label.into(),
            value,
            color,
        }
    }
}

define_widget! {
    pub struct PieChart {
        data: Vec<PieData>,
        fixed_size: f32,
        hole_radius: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let s = if self.fixed_size > 0.0 { self.fixed_size } else { 180.0 };
        Size::new(s, s)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let n = self.data.len();
        if n == 0 { return; }
        let total: f32 = self.data.iter().map(|d| d.value).sum();
        if total <= 0.0 { return; }

        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let chart_r = (frame.w.min(frame.h) * 0.5 - 4.0).max(4.0);
        if chart_r <= 0.0 { return; }

        let tokens = ctx.tokens();
        let text_c = tokens.color_text();
        let axis_c = tokens.color_border();
        let bg_c = tokens.color_bg_container();

        let font_size = (chart_r * 0.17).max(8.0);
        let mut sa = -std::f32::consts::FRAC_PI_2;

        for d in &self.data {
            let a = (d.value / total) * std::f32::consts::TAU;
            let ea = sa + a;
            let ma = sa + a * 0.5;
            let eng = ctx.canvas_2d();
            eng.fill_sector(cx, cy, chart_r, sa, ea, d.color);
            eng.draw_line(cx, cy, cx + chart_r * sa.cos(), cy + chart_r * sa.sin(), text_c, 1.0);

            if a > std::f32::consts::TAU * 0.04 {
                let half_a = a * 0.5;
                let centroid_r = if half_a > 0.001 {
                    chart_r * (2.0 / 3.0) * half_a.sin() / half_a
                } else {
                    chart_r * (2.0 / 3.0)
                };
                let pct = d.value / total * 100.0;
                let label = if pct >= 3.0 { format!("{:.0}%", pct) } else { String::new() };
                if !label.is_empty() {
                    let lx = cx + centroid_r * ma.cos();
                    let ly = cy + centroid_r * ma.sin();
                    let lsz = ctx.measure_text(&label, font_size);
                    let text_rect = Rect::new(lx - lsz.w * 0.5, ly - font_size * 0.8, lsz.w, font_size * 1.6);
                    let text_y = ctx.visual_center_y(text_rect, font_size);
                    ctx.draw_text(&label, Point::new(lx - lsz.w * 0.5, text_y), Color::white(), font_size);
                }
            }
            sa = ea;
        }
        {
            let eng = ctx.canvas_2d();
            eng.draw_line(cx, cy, cx + chart_r * sa.cos(), cy + chart_r * sa.sin(), text_c, 1.0);
        }
        if self.hole_radius > 0.0 {
            let hr = chart_r * self.hole_radius;
            let eng = ctx.canvas_2d();
            eng.fill_circle(cx, cy, hr, bg_c);
            eng.stroke_circle(cx, cy, hr, axis_c, 1.0);

            let center_label = format!("{:.0}", total);
            let cl_fs = hr * 0.6;
            let cl_y = ctx.visual_center_y(Rect::new(cx - hr, cy - hr, hr * 2.0, hr * 2.0), cl_fs);
            let cl_sz = ctx.measure_text(&center_label, cl_fs);
            ctx.draw_text(&center_label, Point::new(cx - cl_sz.w * 0.5, cl_y), text_c, cl_fs);
        }
    }
}

impl Default for PieChart {
    fn default() -> Self {
        Self::new()
    }
}
impl PieChart {
    pub fn new() -> Self {
        Self {
            data: Vec::new(),
            fixed_size: 0.0,
            hole_radius: 0.0,
        }
    }
    pub fn data(mut self, d: Vec<PieData>) -> Self {
        self.data = d;
        self
    }
    pub fn size(mut self, s: f32) -> Self {
        self.fixed_size = s;
        self
    }
    pub fn donut(mut self, r: f32) -> Self {
        self.hole_radius = r.clamp(0.0, 0.9);
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::PieChart {
            data: self.data.clone(),
            fixed_size: self.fixed_size,
            hole_radius: self.hole_radius,
        }
    }
}
