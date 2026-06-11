//! Popconfirm widget — 确认弹窗。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Popconfirm — 点击触发确认弹窗，支持确认/取消。
    pub struct Popconfirm {
        title: String,
        confirm_text: String,
        cancel_text: String,
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
            // 弹窗按钮处理
            if self.visible && pos.x >= 0.0 && pos.y < 0.0 {
                let pop_h = 90.0;
                let rel_y = pos.y + pop_h + 8.0 + 32.0; // 映射到弹窗坐标
                if rel_y >= 0.0 {
                    // Cancel = (w-120 ~ w-8)
                    if pos.x >= 100.0 && pos.x <= 172.0 && rel_y >= 52.0 && rel_y <= 78.0 {
                        self.visible = false;
                        return EventResult::Handled;
                    }
                    // Confirm = (8 ~ 92)
                    if pos.x >= 8.0 && pos.x <= 92.0 && rel_y >= 52.0 && rel_y <= 78.0 {
                        self.visible = false;
                        return EventResult::Handled;
                    }
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let primary = ctx.tokens().color_primary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 触发文字
        ctx.draw_text("Delete", crate::base::Point::new(frame.x + 20.0, frame.y + 6.0),
            ctx.tokens().color_error(), 13.0);

        if self.visible {
            let pop_w = 180.0;
            let pop_h = 90.0;
            let px = frame.x;
            let py = frame.y - pop_h - 8.0;
            let pop_rect = Rect::new(px, py, pop_w, pop_h);
            ctx.fill_rect(pop_rect, bg, r);
            ctx.stroke_rect(pop_rect, border, 1.0, r);

            // 标题
            let title = if self.title.is_empty() { "Are you sure?" } else { &self.title };
            ctx.draw_text(title, crate::base::Point::new(px + 12.0, py + 14.0), text_color, 13.0);

            // 确认按钮
            let confirm = if self.confirm_text.is_empty() { "OK" } else { &self.confirm_text };
            let cancel = if self.cancel_text.is_empty() { "Cancel" } else { &self.cancel_text };
            let btn_r = Some(Radius::uniform(4.0));

            ctx.fill_rect(Rect::new(px + 12.0, py + 54.0, 72.0, 26.0), primary, btn_r);
            ctx.text_center(confirm, Rect::new(px + 12.0, py + 54.0, 72.0, 26.0), Color::white(), 12.0);
            ctx.stroke_rect(Rect::new(px + 96.0, py + 54.0, 72.0, 26.0), border, 1.0, btn_r);
            ctx.text_center(cancel, Rect::new(px + 96.0, py + 54.0, 72.0, 26.0), text_color, 12.0);
        }
    }
}

impl Default for Popconfirm { fn default() -> Self { Self::new() } }

impl Popconfirm {
    pub fn new() -> Self {
        Self { title: String::new(), confirm_text: "OK".to_string(), cancel_text: "Cancel".to_string(), visible: false }
    }
    pub fn title(mut self, t: impl Into<String>) -> Self { self.title = t.into(); self }
}
