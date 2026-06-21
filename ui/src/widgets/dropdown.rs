//! Dropdown widget — 下拉菜单。

use crate::define_widget;
use uix_core::{Rect, Size};
use uix_graphics::Radius;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};

define_widget! {
    /// Dropdown — 点击触发的下拉菜单。
    /// 菜单在 post_render 中渲染为浮层，不触发布局偏移。
    pub struct Dropdown {
        label: String,
        items: Vec<String>,
        open: bool,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::GraphicsEngine>) -> Size {
        // 固定为按钮高度，不随 open 变化，避免布局偏移
        Size::new(160.0, 32.0)
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
                    self.open = false;
                    return EventResult::Handled;
                }
            }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        // 触发按钮（固定高度 32px）
        let btn_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(btn_rect, ctx.tokens().color_primary(), r);
        ctx.text_center(&self.label, btn_rect, uix_graphics::Color::white(), 13.0);
    }

    // 菜单作为浮层渲染（不影响布局定位）
    post_render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        if !self.open { return; }
        let bg = ctx.tokens().color_bg_elevated();
        let border = ctx.tokens().color_border();
        let text_color = ctx.tokens().color_text();
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

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

    dirty_rect => (&self, frame: Rect) -> Rect {
        // 始终包含菜单区域，确保 open 切换时残留像素被清除
        let menu_h = self.items.len() as f32 * 30.0;
        let menu = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
        frame.union(&menu)
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
