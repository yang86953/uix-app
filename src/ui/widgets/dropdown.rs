//! Dropdown widget — 下拉菜单。

use crate::define_widget;
use crate::base::{Rect, Size};
use crate::graphics::Radius;
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Dropdown — 点击触发的下拉菜单。
    pub struct Dropdown {
        label: String,
        items: Vec<String>,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let list_h = if self.open { self.items.len() as f32 * 30.0 } else { 0.0 };
        Size::new(160.0, 32.0 + list_h)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            if pos.y >= 0.0 && pos.y <= 32.0 {
                self.open = !self.open;
                return EventResult::Handled;
            }
            if self.open && pos.y > 32.0 {
                let idx = ((pos.y - 32.0) / 30.0) as usize;
                if idx < self.items.len() {
                    // 点击菜单项后关闭
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
        let _text_secondary = ctx.tokens().color_text_secondary();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 触发按钮
        let btn_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(btn_rect, ctx.tokens().color_primary(), r);
        ctx.text_center(&self.label, btn_rect, crate::graphics::Color::white(), 13.0);

        // 下拉菜单
        if self.open {
            let menu_y = frame.y + 32.0;
            let menu_h = self.items.len() as f32 * 30.0;
            let menu_rect = Rect::new(frame.x, menu_y, frame.w, menu_h);
            ctx.fill_rect(menu_rect, bg, r);
            ctx.stroke_rect(menu_rect, border, 1.0, r);

            for (i, item) in self.items.iter().enumerate() {
                let item_rect = Rect::new(frame.x, menu_y + i as f32 * 30.0, frame.w, 30.0);
                ctx.text_center(item, item_rect, text_color, 13.0);
            }
        }
    }
}

impl Default for Dropdown { fn default() -> Self { Self::new("Menu") } }

impl Dropdown {
    pub fn new(label: impl Into<String>) -> Self {
        Self { label: label.into(), items: Vec::new(), open: false }
    }
    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }
}
