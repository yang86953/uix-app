//! List widget — 列表组件，Ant Design 风格。
//!
//! 支持列表项渲染、header/footer、bordered、size 等选项。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};

/// 列表尺寸。
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ListSize {
    Small,
    Default,
    Large,
}

impl ListSize {
    pub fn item_height(&self) -> f32 {
        match self { Self::Small => 32.0, Self::Default => 40.0, Self::Large => 48.0 }
    }
}

/// List — 列表组件。
define_widget! {
    pub struct List {
        header: String,
        footer: String,
        bordered: bool,
        list_size: ListSize,
        items: Vec<String>,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let h = self.items.len() as f32 * self.list_size.item_height()
            + if self.header.is_empty() { 0.0 } else { 40.0 }
            + if self.footer.is_empty() { 0.0 } else { 40.0 };
        Size::new(400.0, h.max(100.0))
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let item_h = self.list_size.item_height();
        let r = Radius::uniform(ctx.tokens().border_radius());
        let mut y = frame.y;

        // 主体背景
        ctx.fill_rect(frame, bg, Some(r));
        if self.bordered {
            ctx.stroke_rect(frame, border, 1.0, Some(r));
        }

        // Header
        if !self.header.is_empty() {
            ctx.draw_text(&self.header, Point::new(frame.x + 16.0, y + 10.0), text_sec, 13.0);
            ctx.fill_rect(Rect::new(frame.x, y + item_h, frame.w, 1.0), border, None);
            y += item_h;
        }

        // Items
        for (i, item) in self.items.iter().enumerate() {
            ctx.draw_text(item, Point::new(frame.x + 16.0, y + 10.0), text, 14.0);
            if i < self.items.len() - 1 {
                ctx.fill_rect(Rect::new(frame.x + 16.0, y + item_h - 1.0, frame.w - 32.0, 1.0), border, None);
            }
            y += item_h;
        }

        // Footer
        if !self.footer.is_empty() {
            if !self.items.is_empty() {
                ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
            }
            ctx.draw_text(&self.footer, Point::new(frame.x + 16.0, y + 10.0), text_sec, 13.0);
        }
    }
}

impl List {
    pub fn new() -> Self {
        Self { header: String::new(), footer: String::new(), bordered: true, list_size: ListSize::Default, items: Vec::new() }
    }
    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect(); self
    }
    pub fn header(mut self, h: &str) -> Self { self.header = h.to_string(); self }
    pub fn footer(mut self, f: &str) -> Self { self.footer = f.to_string(); self }
    pub fn bordered(mut self, v: bool) -> Self { self.bordered = v; self }
    pub fn size(mut self, s: ListSize) -> Self { self.list_size = s; self }
}

impl Default for List { fn default() -> Self { Self::new() } }
