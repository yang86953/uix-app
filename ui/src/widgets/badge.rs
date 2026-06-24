use crate::define_widget;
use uix_core::{Rect, Size};
use uix_graphics::{Color, Radius};
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BadgeStatus { Success, Processing, Default, Error, Warning }

define_widget! {
    pub struct Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        _size: f32,
        status: Option<BadgeStatus>,
        show_zero: bool,
        text: String,
        offset_x: f32,
        offset_y: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        if self.dot || self.status.is_some() {
            Size::new(10.0, 10.0)
        } else if self.count > 0 || (self.count == 0 && self.show_zero) {
            let text = format!("{}", self.count.min(self.max));
            let w = (text.len() as f32) * 7.0 + 12.0;
            Size::new(w.max(20.0), 20.0)
        } else if !self.text.is_empty() {
            let w = self.text.len() as f32 * 7.0 + 12.0;
            Size::new(w, 20.0)
        } else {
            Size::zero()
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let actual_frame = Rect::new(frame.x + self.offset_x, frame.y + self.offset_y, frame.w, frame.h);

        if let Some(status) = self.status {
            let sc = match status {
                BadgeStatus::Success => ctx.tokens().color_success(),
                BadgeStatus::Processing => ctx.tokens().color_primary(),
                BadgeStatus::Default => ctx.tokens().color_text_quaternary(),
                BadgeStatus::Error => ctx.tokens().color_error(),
                BadgeStatus::Warning => ctx.tokens().color_warning(),
            };
            let r = actual_frame.h * 0.5;
            ctx.fill_circle(actual_frame.x + r, actual_frame.y + r, r, sc);
            return;
        }

        if self.count == 0 && !self.show_zero && self.text.is_empty() { return; }
        let bg = self.color.unwrap_or(ctx.tokens().color_error());
        let display = self.count.min(self.max);
        if self.dot {
            let r = actual_frame.h * 0.5;
            ctx.fill_circle(actual_frame.x + r, actual_frame.y + r, r, bg);
        } else if !self.text.is_empty() {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            ctx.text_center(&self.text, actual_frame, Color::white(), 11.0);
        } else {
            let r = Some(Radius::uniform(actual_frame.h * 0.5));
            ctx.fill_rect(actual_frame, bg, r);
            let text = format!("{}", display);
            let over = if self.count > self.max { "+" } else { "" };
            let label = format!("{}{}", text, over);
            ctx.text_center(&label, actual_frame, Color::white(), 11.0);
        }
    }
}

impl Default for Badge { fn default() -> Self { Self::new() } }

impl Badge {
    pub fn new() -> Self {
        Self { count: 0, max: 99, dot: false, color: None, _size: 16.0, status: None, show_zero: false, text: String::new(), offset_x: 0.0, offset_y: 0.0 }
    }
    pub fn count(mut self, n: i32) -> Self { self.count = n; self }
    pub fn max(mut self, n: i32) -> Self { self.max = n; self }
    pub fn dot(mut self) -> Self { self.dot = true; self.count = 1; self }
    pub fn color(mut self, c: Color) -> Self { self.color = Some(c); self }
    pub fn status(mut self, s: BadgeStatus) -> Self { self.status = Some(s); self }
    pub fn show_zero(mut self, v: bool) -> Self { self.show_zero = v; self }
    pub fn text(mut self, t: &str) -> Self { self.text = t.to_string(); self }
    pub fn offset(mut self, x: f32, y: f32) -> Self { self.offset_x = x; self.offset_y = y; self }
}
