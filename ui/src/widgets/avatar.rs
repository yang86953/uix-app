//! Avatar — circular avatar with initials/text.

use crate::define_widget;
use uix_graphics::Color;
use uix_core::{Point, Rect, Size};
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

define_widget! {
    pub struct Avatar {
        text: String,
        size: f32,
        bg_color: Option<Color>,
        text_color: Option<Color>,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        Size::new(self.size, self.size)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary_bg = ctx.tokens().color_primary_bg();
        let primary = ctx.tokens().color_primary();

        let cx = frame.x + frame.w * 0.5;
        let cy = frame.y + frame.h * 0.5;
        let r = frame.w.min(frame.h) * 0.5;

        let bg = self.bg_color.unwrap_or(primary_bg);
        let tc = self.text_color.unwrap_or(primary);

        ctx.fill_circle(cx, cy, r, bg);

        if !self.text.is_empty() {
            let font_size = self.size * 0.45;
            let sz = ctx.measure_text(&self.text, font_size);
            let tx = cx - sz.w * 0.5;
            let ty = cy - sz.h * 0.5;
            ctx.draw_text(&self.text, Point::new(tx, ty), tc, font_size);
        }
    }
}

impl Default for Avatar { fn default() -> Self { Self::new("") } }

impl Avatar {
    pub fn new(text: impl Into<String>) -> Self {
        Self { text: text.into(), size: 32.0, bg_color: None, text_color: None }
    }
    pub fn size(mut self, s: f32) -> Self { self.size = s; self }
    pub fn bg(mut self, c: Color) -> Self { self.bg_color = Some(c); self }
    pub fn text_color(mut self, c: Color) -> Self { self.text_color = Some(c); self }
}
