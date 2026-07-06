//! LineChart — line chart with grid lines and data point markers.

use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::Color;
use crate::ui::WidgetTree;

#[derive(Debug, Clone)]
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

define_widget! {
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

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let w = if self.fixed_width > 0.0 { self.fixed_width } else { 300.0 };
        let h = if self.fixed_height > 0.0 { self.fixed_height } else { 200.0 };
        Size::new(w, h)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let n = self.data.len();
        if n < 2 || frame.w <= 0.0 || frame.h <= 0.0 { return; }

        let data_min = self.data.iter().map(|d| d.value).fold(f32::MAX, f32::min);
        let data_max = self.data.iter().map(|d| d.value).fold(f32::MIN, f32::max);
        let r_min = if self.auto_min { data_min } else { 0.0 };
        let r_max = if self.max_value > 0.0 { self.max_value } else { data_max };
        let range = (r_max - r_min).max(1.0);

        let tokens = ctx.tokens();
        let lc = self.line_color.unwrap_or(tokens.color_text());
        let lbc = tokens.color_text_secondary();
        let ac = tokens.color_border();
        let bg = tokens.color_bg_container();
        let _text_c = tokens.color_text();

        let y_label_w = 36.0;
        let x_label_h = 14.0;
        let chart_x = frame.x + y_label_w;
        let chart_w = (frame.w - y_label_w).max(1.0);
        let chart_h = (frame.h - x_label_h).max(1.0);
        let chart_y = frame.y;

        let map_y = |v: f32| chart_y + chart_h - ((v - r_min) / range) * chart_h;

        let step = if n > 1 { chart_w / (n - 1) as f32 } else { chart_w };

        if self.show_grid {
            let gl = 4.max((chart_h / 30.0) as usize);
            for i in 0..gl {
                let t = (i as f32 + 1.0) / gl as f32;
                let gy = chart_y + chart_h * (1.0 - t);
                ctx.fill_rect(Rect::new(chart_x, gy, chart_w, 0.5), ac, None);
                let val = r_min + range * t;
                let label = if val == val.trunc() { format!("{:.0}", val) } else { format!("{:.1}", val) };
                let y_label_rect = Rect::new(frame.x, gy - 6.0, y_label_w - 2.0, 12.0);
                let yly = ctx.visual_center_y(y_label_rect, 9.0);
                let lsz = ctx.measure_text(&label, 9.0);
                ctx.draw_text(&label, Point::new(chart_x - lsz.w - 4.0, yly), lbc, 9.0);
            }
        }

        let bl = map_y(r_min);
        ctx.fill_rect(Rect::new(chart_x, bl, chart_w, 1.0), ac, None);

        let pts: Vec<crate::core::Point> = self.data.iter().enumerate().map(|(i, d)|
            crate::core::Point::new(chart_x + i as f32 * step, map_y(d.value))
        ).collect();

        let lw = self.line_width.max(1.0);
        let half = (lw * 0.5).floor() as i32;
        for s in 0..n - 1 {
            for o in -half..=half {
                let o = o as f32;
                ctx.canvas_2d().draw_line(pts[s].x, pts[s].y + o, pts[s + 1].x, pts[s + 1].y + o, lc, 1.0);
            }
        }

        if self.show_dots && self.dot_radius > 0.0 {
            for pt in &pts {
                ctx.fill_circle(pt.x, pt.y, self.dot_radius, lc);
                ctx.fill_circle(pt.x, pt.y, (self.dot_radius - 1.5).max(0.5), bg);
            }
        }

        for (i, d) in self.data.iter().enumerate() {
            let x = chart_x + i as f32 * step;
            let sz = ctx.measure_text(&d.label, 10.0);
            let lx = (x - sz.w * 0.5).max(chart_x).min(chart_x + chart_w - sz.w);
            let label_rect = Rect::new(lx, bl + 2.0, sz.w, x_label_h - 2.0);
            let ly = ctx.visual_center_y(label_rect, 10.0);
            ctx.draw_text(&d.label, crate::core::Point::new(lx, ly), lbc, 10.0);
        }
    }
}

impl Default for LineChart {
    fn default() -> Self {
        Self::new()
    }
}
impl LineChart {
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
        self.fixed_width = w;
        self
    }
    pub fn height(mut self, h: f32) -> Self {
        self.fixed_height = h;
        self
    }
    pub fn line_color(mut self, c: Color) -> Self {
        self.line_color = Some(c);
        self
    }
    pub fn max_value(mut self, v: f32) -> Self {
        self.max_value = v;
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
        self.line_width = w;
        self
    }
}
