//! PieChart — pie / donut chart with proportional circular sectors.

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
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

component! {
    pub struct PieChart {
        data: Vec<PieData>,
        fixed_size: f32,
        hole_radius: f32,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let slices = self.normalized_slices();
        if slices.is_empty() || frame.w <= 0.0 || frame.h <= 0.0 { return; }

        let legend_w = if slices.iter().any(|slice| !slice.data.label.trim().is_empty())
            && frame.w >= 120.0
        {
            (frame.w * 0.35).clamp(70.0, 140.0)
        } else {
            0.0
        };
        let chart_area_w = frame.w - legend_w;
        let chart_r = (chart_area_w.min(frame.h) * 0.5 - 4.0).max(0.0);
        if chart_r <= 0.0 { return; }

        let cx = frame.x + chart_area_w * 0.5;
        let cy = frame.y + frame.h * 0.5;

        let tokens = ctx.tokens();
        let text_c = tokens.color_text();
        let axis_c = tokens.color_border();
        let bg_c = tokens.color_bg_container();

        let font_size = (chart_r * 0.17).max(8.0);
        let mut sa = -std::f32::consts::FRAC_PI_2;
        ctx.push_clip(frame);

        for slice in &slices {
            let d = slice.data;
            let a = slice.fraction * std::f32::consts::TAU;
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
                let pct = slice.fraction * 100.0;
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

            let center_label = Self::format_value(Self::total(&slices));
            let cl_fs = hr * 0.6;
            let cl_y = ctx.visual_center_y(Rect::new(cx - hr, cy - hr, hr * 2.0, hr * 2.0), cl_fs);
            let cl_sz = ctx.measure_text(&center_label, cl_fs);
            ctx.draw_text(&center_label, Point::new(cx - cl_sz.w * 0.5, cl_y), text_c, cl_fs);
        }

        if legend_w > 0.0 {
            let legend_x = frame.x + chart_area_w + 8.0;
            let row_h = 18.0;
            for (index, slice) in slices.iter().enumerate() {
                let y = frame.y + 4.0 + index as f32 * row_h;
                if y + row_h > frame.y + frame.h {
                    break;
                }
                ctx.fill_rect(
                    Rect::new(legend_x, y + 4.0, 8.0, 8.0),
                    slice.data.color,
                    None,
                );
                let percent = format!("{}%", Self::format_value(f64::from(slice.fraction) * 100.0));
                let label = if slice.data.label.trim().is_empty() {
                    percent
                } else {
                    format!("{} {percent}", slice.data.label.trim())
                };
                ctx.draw_text(&label, Point::new(legend_x + 12.0, y + 2.0), text_c, 10.0);
            }
        }
        ctx.pop_clip();
    }
}

#[derive(Debug)]
struct PieSlice<'a> {
    data: &'a PieData,
    fraction: f32,
}

impl Default for PieChart {
    fn default() -> Self {
        Self::new()
    }
}
impl PieChart {
    const DEFAULT_SIZE: f32 = 180.0;

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
        self.fixed_size = if s.is_finite() && s > 0.0 { s } else { 0.0 };
        self
    }
    pub fn donut(mut self, r: f32) -> Self {
        self.hole_radius = if r.is_finite() {
            r.clamp(0.0, 0.9)
        } else {
            0.0
        };
        self
    }

    fn intrinsic_size(&self) -> Size {
        let size = if self.fixed_size > 0.0 {
            self.fixed_size
        } else {
            Self::DEFAULT_SIZE
        };
        Size::new(size, size)
    }

    fn normalized_slices(&self) -> Vec<PieSlice<'_>> {
        let total = self
            .data
            .iter()
            .filter(|item| item.value.is_finite() && item.value > 0.0)
            .map(|item| f64::from(item.value))
            .sum::<f64>();
        if !total.is_finite() || total <= 0.0 {
            return Vec::new();
        }
        self.data
            .iter()
            .filter(|item| item.value.is_finite() && item.value > 0.0)
            .map(|data| PieSlice {
                data,
                fraction: (f64::from(data.value) / total) as f32,
            })
            .collect()
    }

    fn total(slices: &[PieSlice<'_>]) -> f64 {
        slices.iter().map(|slice| f64::from(slice.data.value)).sum()
    }

    fn format_value(value: f64) -> String {
        if value == value.trunc() {
            format!("{value:.0}")
        } else {
            format!("{value:.1}")
        }
    }

    #[cfg(test)]
    pub(crate) fn slices_for_test(&self) -> Vec<(String, f32)> {
        self.normalized_slices()
            .into_iter()
            .map(|slice| (slice.data.label.clone(), slice.fraction))
            .collect()
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.data = next.data;
        self.fixed_size = next.fixed_size;
        self.hole_radius = next.hole_radius;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::PieChart {
            data: self.data.clone(),
            fixed_size: self.fixed_size,
            hole_radius: self.hole_radius,
        }
    }
}
