//! Label widget — displays text.

use crate::graphics::{Color, GraphicsEngine, Rect, Size, Point};
use crate::ui::widget::{Widget, WidgetTree};
use crate::ui::render_context::RenderContext;

pub struct Label {
    pub text: String,
    pub font_size: f32,
    pub color: Color,
    pub fixed_width: Option<f32>,
    pub fixed_height: Option<f32>,
}

impl Label {
    pub fn new(text: &str, color: Color) -> Self {
        Self {
            text: text.to_string(),
            font_size: 12.0,
            color,
            fixed_width: None,
            fixed_height: None,
        }
    }

    pub fn font_size(mut self, s: f32) -> Self { self.font_size = s; self }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
}

impl Widget for Label {
    fn preferred_size(&self, _engine: Option<&dyn GraphicsEngine>) -> Size {
        if let (Some(w), Some(h)) = (self.fixed_width, self.fixed_height) {
            Size::new(w, h)
        } else {
            let len = self.text.len() as f32;
            Size::new(len * 6.0 + 4.0, self.font_size + 4.0)
        }
    }

    fn render(&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        ctx.draw_text(
            &self.text,
            Point::new(frame.x + 2.0, frame.y + 2.0),
            self.color,
            self.font_size,
        );
    }
}
