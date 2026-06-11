//! Select widget — 下拉选择器。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Select — 下拉选择框。
    pub struct Select {
        options: Vec<String>,
        selected: usize,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let w = self.options.iter().map(|o| o.len() as f32 * 9.0 + 32.0).max_by(|a,b| a.partial_cmp(b).unwrap()).unwrap_or(150.0).max(120.0);
        let list_h = if self.open { self.options.len() as f32 * 28.0 } else { 0.0 };
        Size::new(w, 32.0 + list_h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            // 触发区域
            if pos.y >= 0.0 && pos.y <= 32.0 {
                self.open = !self.open;
                return EventResult::Handled;
            }
            // 下拉列表点击
            if self.open && pos.y > 32.0 {
                let idx = ((pos.y - 32.0) / 28.0) as usize;
                if idx < self.options.len() {
                    self.selected = idx;
                    self.open = false;
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let _fill = ctx.tokens().color_fill_tertiary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 选择框
        let box_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(box_rect, bg, r);
        ctx.stroke_rect(box_rect, border, 1.0, r);

        let selected_text = if self.selected < self.options.len() { &self.options[self.selected] } else { "" };
        ctx.draw_text(selected_text, crate::base::Point::new(frame.x + 10.0, frame.y + 8.0), text_color, 13.0);
        // 下拉箭头
        let arrow = if self.open { "▲" } else { "▼" };
        ctx.draw_text(arrow, crate::base::Point::new(frame.x + frame.w - 18.0, frame.y + 8.0), text_secondary, 10.0);

        // 下拉列表
        if self.open {
            let list_y = frame.y + 32.0;
            let list_h = self.options.len() as f32 * 28.0;
            let list_rect = Rect::new(frame.x, list_y, frame.w, list_h);
            ctx.fill_rect(list_rect, bg, r);
            ctx.stroke_rect(list_rect, border, 1.0, r);

            for (i, opt) in self.options.iter().enumerate() {
                let item_rect = Rect::new(frame.x, list_y + i as f32 * 28.0, frame.w, 28.0);
                if i == self.selected {
                    ctx.fill_rect(item_rect, ctx.tokens().color_primary_bg(), None);
                    ctx.draw_text(opt, crate::base::Point::new(frame.x + 10.0, list_y + i as f32 * 28.0 + 6.0), ctx.tokens().color_primary(), 13.0);
                } else {
                    ctx.draw_text(opt, crate::base::Point::new(frame.x + 10.0, list_y + i as f32 * 28.0 + 6.0), text_color, 13.0);
                }
            }
        }
    }
}

impl Default for Select { fn default() -> Self { Self::new() } }

impl Select {
    pub fn new() -> Self {
        Self { options: Vec::new(), selected: 0, open: false }
    }
    pub fn options(mut self, opts: Vec<impl Into<String>>) -> Self {
        self.options = opts.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn selected(mut self, idx: usize) -> Self { self.selected = idx; self }
}
