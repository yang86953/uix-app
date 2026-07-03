//! Dropdown widget — 下拉菜单。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::{EventResult, WidgetEvent, WidgetTree};
use uix_graphics::Radius;
use uix_platform::{Rect, Size};

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

    // 菜单展开时扩展 hit_test 区域，使浮层中的菜单项可点击
    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.open {
            let menu_h = self.items.len() as f32 * 30.0;
            Rect::new(frame.x, frame.y, frame.w, 32.0 + menu_h)
        } else {
            frame
        }
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
        // 绘制浮层阴影
        let shadow = ctx.tokens().box_shadow_secondary();
        ctx.draw_box_shadow(menu_rect, shadow.layer_1.2, shadow.layer_1.0, shadow.layer_1.1, shadow.layer_1.3, r);
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
        let expanded = frame.union(&menu);
        // 扩展脏区域覆盖阴影边界（blur + 安全边距）
        let expand = 8.0;
        Rect::new(expanded.x - expand, expanded.y - expand, expanded.w + expand * 2.0, expanded.h + expand * 2.0)
    }
}

impl Default for Dropdown {
    fn default() -> Self {
        Self::new("Menu")
    }
}

impl Dropdown {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
            open: false,
        }
    }
    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }
}
