//! Label widget — displays text.

use crate::graphics::{Color, FontHandle, GraphicsEngine, Point, Rect, Size, TextLayoutOptions};
use crate::ui::render_context::RenderContext;
use crate::define_widget;
use crate::ui::widget::WidgetTree;

define_widget! {
    pub struct Label {
        pub text: String,
        pub font_size: f32,
        pub color: Color,
        pub fixed_width: Option<f32>,
        pub fixed_height: Option<f32>,
    }

    preferred_size => (&self, engine: Option<&dyn GraphicsEngine>) -> Size {
        if let (Some(w), Some(h)) = (self.fixed_width, self.fixed_height) {
            Size::new(w, h)
        } else {
            // 使用 TTF 度量计算实际尺寸，回退到位图字体
            if let Some(eng) = engine {
                let opts = TextLayoutOptions {
                    max_width: f32::MAX,
                    line_height: self.font_size + 2.0,
                    word_wrap: false,
                    h_align: crate::graphics::HAlign::Left,
                    v_align: crate::graphics::VAlign::Top,
                    font_size: self.font_size,
                };
                let sz = eng.measure_text(&FontHandle::default(), &self.text, &opts);
                if sz.w > 0.0 && sz.h > 0.0 {
                    return Size::new(sz.w + 4.0, sz.h.max(self.font_size + 4.0));
                }
            }
            // Fallback: bitmap font estimate
            let len = self.text.len() as f32;
            Size::new(len * 6.0 + 4.0, self.font_size + 4.0)
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        // Vertically center text within the frame
        let text_y = frame.y + (frame.h - self.font_size) * 0.5;
        ctx.draw_text(
            &self.text,
            Point::new(frame.x + 2.0, text_y.max(frame.y)),
            self.color,
            self.font_size,
        );
    }
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

    pub fn font_size(mut self, s: f32) -> Self {
        self.font_size = s;
        self
    }

    pub fn size(mut self, w: f32, h: f32) -> Self {
        self.fixed_width = Some(w);
        self.fixed_height = Some(h);
        self
    }
}
