//! Radio widget — 单选组。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Radio — 单选按钮组。
    pub struct Radio {
        options: Vec<String>,
        selected: usize,
        item_h: f32,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let w = self.options.iter().map(|o| o.len() as f32 * 9.0 + 30.0).sum::<f32>().max(120.0);
        Size::new(w, self.item_h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let mut cum_x = 0.0f32;
            for (i, opt) in self.options.iter().enumerate() {
                let w = opt.len() as f32 * 9.0 + 30.0;
                if pos.x >= cum_x && pos.x <= cum_x + w && pos.y >= 0.0 && pos.y <= self.item_h {
                    self.selected = i;
                    return EventResult::Handled;
                }
                cum_x += w;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text_color = ctx.tokens().color_text();
        let border = ctx.tokens().color_border();
        let cy = frame.y + self.item_h * 0.5;
        let mut x = frame.x;

        for (i, opt) in self.options.iter().enumerate() {
            let r = 6.0;
            ctx.stroke_rect(Rect::new(x + 1.0, cy - r, r * 2.0, r * 2.0), border, 1.5, Some(Radius::uniform(r)));
            if i == self.selected {
                ctx.fill_circle(x + r + 1.0, cy, 3.5, primary);
            }
            ctx.draw_text(opt, crate::base::Point::new(x + 20.0, cy - 6.0), text_color, 13.0);
            x += opt.len() as f32 * 9.0 + 30.0;
        }
    }
}

impl Default for Radio { fn default() -> Self { Self::new() } }

impl Radio {
    pub fn new() -> Self {
        Self { options: Vec::new(), selected: 0, item_h: 24.0 }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn selected(mut self, idx: usize) -> Self { self.selected = idx; self }
}
