//! Popover widget — 点击弹出卡片。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Popover — 点击触发弹出卡片，显示标题和内容。
    pub struct Popover {
        title: String,
        content: String,
        visible: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::new(80.0, 28.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.x >= 0.0 && pos.x <= 80.0 && pos.y >= 0.0 && pos.y <= 28.0 {
                self.visible = !self.visible;
                return EventResult::Handled;
            }
            // 点击弹窗外区域关闭弹窗（简化：弹窗区域内不关闭）
            let pop_w = 200.0;
            let pop_h = 80.0;
            let pop_y = -pop_h - 8.0;
            if self.visible && !(pos.x >= 0.0 && pos.x <= pop_w && pos.y >= pop_y && pos.y <= pop_y + pop_h) {
                self.visible = false;
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 触发区域（虚线边框）
        ctx.stroke_rect(frame, border, 1.0, r);
        ctx.text_center("Popover", frame, text_secondary, 12.0);

        // 弹出卡片（在上方）
        if self.visible {
            let pop_w = 200.0;
            let pop_h = 80.0;
            let px = frame.x;
            let py = frame.y - pop_h - 8.0;
            let pop_rect = Rect::new(px, py, pop_w, pop_h);
            ctx.fill_rect(pop_rect, bg, r);
            ctx.stroke_rect(pop_rect, border, 1.0, r);

            if !self.title.is_empty() {
                ctx.draw_text(&self.title, crate::base::Point::new(px + 12.0, py + 10.0), text_color, 14.0);
                ctx.fill_rect(Rect::new(px + 12.0, py + 32.0, pop_w - 24.0, 1.0), border, None);
            }
            let content_y = py + if self.title.is_empty() { 12.0 } else { 40.0 };
            ctx.draw_text(&self.content, crate::base::Point::new(px + 12.0, content_y), text_secondary, 12.0);
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        if self.visible {
            let pop_w = 200.0;
            let pop_h = 80.0;
            let py = frame.y - pop_h - 8.0;
            let pop = Rect::new(frame.x, py, pop_w, pop_h);
            frame.union(&pop)
        } else {
            frame
        }
    }
}

impl Default for Popover { fn default() -> Self { Self::new("") } }

impl Popover {
    pub fn new(content: impl Into<String>) -> Self {
        Self { title: String::new(), content: content.into(), visible: false }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self { self.title = t.into(); self }
}
