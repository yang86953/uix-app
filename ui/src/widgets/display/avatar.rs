//! Avatar — circular avatar with initials/text.

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;
use uix_graphics::Color;
use uix_platform::{Rect, Size};

define_widget! {
    pub struct Avatar {
        text: String,
        size: f32,
        bg_color: Option<Color>,
        text_color: Option<Color>,
        square: bool,
        src: String,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        Size::new(self.size, self.size)
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary_bg = ctx.tokens().color_primary_bg();
        let primary = ctx.tokens().color_primary();
        let bg = self.bg_color.unwrap_or(primary_bg);
        let tc = self.text_color.unwrap_or(primary);
        let r = if self.square { Some(uix_graphics::Radius::uniform(ctx.tokens().border_radius_sm())) } else { None };

        if self.square {
            ctx.fill_rect(frame, bg, r);
        } else {
            let cx = frame.x + frame.w * 0.5;
            let cy = frame.y + frame.h * 0.5;
            let cr = frame.w.min(frame.h) * 0.5;
            ctx.fill_circle(cx, cy, cr, bg);
        }

        if !self.text.is_empty() {
            let font_size = self.size * 0.45;
            ctx.text_center(&self.text, frame, tc, font_size);
        }
    }
}

impl Default for Avatar {
    fn default() -> Self {
        Self::new("")
    }
}

impl Avatar {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            size: 32.0,
            bg_color: None,
            text_color: None,
            square: false,
            src: String::new(),
        }
    }
    pub fn size(mut self, s: f32) -> Self {
        self.size = s;
        self
    }
    pub fn bg(mut self, c: Color) -> Self {
        self.bg_color = Some(c);
        self
    }
    pub fn text_color(mut self, c: Color) -> Self {
        self.text_color = Some(c);
        self
    }
    pub fn square(mut self, v: bool) -> Self {
        self.square = v;
        self
    }
    pub fn src(mut self, s: &str) -> Self {
        self.src = s.to_string();
        self
    }
}
