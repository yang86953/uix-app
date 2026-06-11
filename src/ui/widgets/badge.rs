//! Badge widget — 徽章/红点/计数标记。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::WidgetTree;

define_widget! {
    /// Badge — 右上角标记，支持数字计数和红点模式。
    pub struct Badge {
        count: i32,
        max: i32,
        dot: bool,
        color: Option<Color>,
        _size: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        if self.dot {
            Size::new(10.0, 10.0)
        } else if self.count > 0 {
            let text = format!("{}", self.count.min(self.max));
            let w = (text.len() as f32) * 7.0 + 12.0;
            Size::new(w.max(20.0), 20.0)
        } else {
            Size::zero()
        }
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if self.count == 0 { return; }
        let bg = self.color.unwrap_or(ctx.tokens().color_error());
        let display = self.count.min(self.max);
        if self.dot {
            let r = frame.h * 0.5;
            ctx.fill_circle(frame.x + r, frame.y + r, r, bg);
        } else {
            let r = Some(Radius::uniform(frame.h * 0.5));
            ctx.fill_rect(frame, bg, r);
            let text = format!("{}", display);
            let over = if self.count > self.max { "+" } else { "" };
            let label = format!("{}{}", text, over);
            ctx.text_center(&label, frame, crate::graphics::Color::white(), 11.0);
        }
    }
}

impl Default for Badge { fn default() -> Self { Self::new() } }

impl Badge {
    pub fn new() -> Self {
        Self { count: 0, max: 99, dot: false, color: None, _size: 16.0 }
    }
    pub fn count(mut self, n: i32) -> Self { self.count = n; self }
    pub fn max(mut self, n: i32) -> Self { self.max = n; self }
    pub fn dot(mut self) -> Self { self.dot = true; self.count = 1; self }
    pub fn color(mut self, c: Color) -> Self { self.color = Some(c); self }
}
