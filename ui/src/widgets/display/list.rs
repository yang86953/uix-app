//! List widget — 列表组件，Ant Design 风格。
//!
//! 支持列表项渲染、header/footer、bordered、size 等选项。

use crate::define_widget;
use crate::render_context::RenderContext;
use crate::widget::WidgetTree;
use uix_graphics::Radius;
use uix_platform::{Point, Rect, Size};

/// 列表尺寸。
/// （已统一为 uix_platform::ControlSize，保留别名以兼容旧代码。）
pub use uix_platform::ControlSize as ListSize;

/// List 尺寸对应的行高。
pub fn list_item_height(size: ListSize) -> f32 {
    match size {
        ListSize::Small => 32.0,
        ListSize::Medium => 40.0,
        ListSize::Large => 48.0,
    }
}

// List — 列表组件。
define_widget! {
    pub struct List {
        header: String,
        footer: String,
        bordered: bool,
        list_size: ListSize,
        items: Vec<String>,
        load_more_text: String,
    }

    preferred_size => (&self, _engine: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> Size {
        let h = self.items.len() as f32 * list_item_height(self.list_size)
            + if self.header.is_empty() { 0.0 } else { 40.0 }
            + if self.footer.is_empty() { 0.0 } else { 40.0 }
            + if self.load_more_text.is_empty() { 0.0 } else { 40.0 };
        Size::new(400.0, h.max(100.0))
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let item_h = list_item_height(self.list_size);
        let r = Radius::uniform(ctx.tokens().border_radius());
        let mut y = frame.y;

        // 主体背景
        ctx.fill_rect(frame, bg, Some(r));
        if self.bordered {
            ctx.stroke_rect(frame, border, 1.0, Some(r));
        }

        // Header
        if !self.header.is_empty() {
            let header_rect = Rect::new(frame.x, y, frame.w, item_h);
            let hy = ctx.visual_center_y(header_rect, 13.0);
            ctx.draw_text(&self.header, Point::new(frame.x + 16.0, hy), text_sec, 13.0);
            ctx.fill_rect(Rect::new(frame.x, y + item_h, frame.w, 1.0), border, None);
            y += item_h;
        }

        // Items
        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(frame.x, y, frame.w, item_h);
            let iy = ctx.visual_center_y(item_rect, 14.0);
            ctx.draw_text(item, Point::new(frame.x + 16.0, iy), text, 14.0);
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
            let footer_rect = Rect::new(frame.x, y, frame.w, item_h);
            let fy = ctx.visual_center_y(footer_rect, 13.0);
            ctx.draw_text(&self.footer, Point::new(frame.x + 16.0, fy), text_sec, 13.0);
            y += item_h;
        }

        // Load more
        if !self.load_more_text.is_empty() {
            let load_rect = Rect::new(frame.x, y, frame.w, 40.0);
            let ly = ctx.visual_center_y(load_rect, 14.0);
            ctx.fill_rect(load_rect, bg, None);
            ctx.stroke_rect(load_rect, border, 1.0, Some(r));
            let text_w = ctx.measure_text(&self.load_more_text, 14.0).w;
            ctx.draw_text(&self.load_more_text, Point::new(frame.x + (frame.w - text_w) * 0.5, ly), ctx.tokens().color_primary(), 14.0);
        }
    }
}

impl List {
    pub fn new() -> Self {
        Self {
            header: String::new(),
            footer: String::new(),
            bordered: true,
            list_size: ListSize::Medium,
            items: Vec::new(),
            load_more_text: String::new(),
        }
    }
    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }
    pub fn header(mut self, h: &str) -> Self {
        self.header = h.to_string();
        self
    }
    pub fn footer(mut self, f: &str) -> Self {
        self.footer = f.to_string();
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn size(mut self, s: ListSize) -> Self {
        self.list_size = s;
        self
    }
    pub fn load_more(mut self, text: impl Into<String>) -> Self {
        self.load_more_text = text.into();
        self
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}
