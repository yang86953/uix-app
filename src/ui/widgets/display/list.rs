//! List widget — 列表组件，Ant Design 风格。
//!
//! 支持列表项渲染、header/footer、bordered、size 等选项。

use crate::component;
use crate::core::{Constraints, EdgeInsets, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::native::traits::input::ControlSize;
use crate::ui::{SnapshotFields, WidgetTree};

/// List 尺寸对应的行高。
pub fn list_item_height(size: ControlSize) -> f32 {
    match size {
        ControlSize::Small => 32.0,
        ControlSize::Medium => 40.0,
        ControlSize::Large => 48.0,
    }
}

// List — 列表组件。
component! {
    pub struct List {
        header: String,
        footer: String,
        bordered: bool,
        list_size: ControlSize,
        items: Vec<String>,
        load_more_text: String,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
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
            let header_rect = Rect::new(frame.x, y, frame.w, item_h)
                .inset(EdgeInsets::new(16.0, 0.0, 16.0, 0.0));
            ctx.draw_text_in_frame(&self.header, header_rect, text_sec, 13.0);
            ctx.fill_rect(Rect::new(frame.x, y + item_h, frame.w, 1.0), border, None);
            y += item_h;
        }

        // Items
        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(frame.x, y, frame.w, item_h)
                .inset(EdgeInsets::new(16.0, 0.0, 16.0, 0.0));
            ctx.draw_text_in_frame(item, item_rect, text, 14.0);
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
            let footer_rect = Rect::new(frame.x, y, frame.w, item_h)
                .inset(EdgeInsets::new(16.0, 0.0, 16.0, 0.0));
            ctx.draw_text_in_frame(&self.footer, footer_rect, text_sec, 13.0);
            y += item_h;
        }

        // Load more
        if !self.load_more_text.is_empty() {
            let load_rect = Rect::new(frame.x, y, frame.w, 40.0);
            ctx.fill_rect(load_rect, bg, None);
            ctx.stroke_rect(load_rect, border, 1.0, Some(r));
            ctx.text_center(&self.load_more_text, load_rect, ctx.tokens().color_primary(), 14.0);
        }
    }
}

impl List {
    fn intrinsic_size(&self) -> Size {
        let h = self.items.len() as f32 * list_item_height(self.list_size)
            + if self.header.is_empty() { 0.0 } else { 40.0 }
            + if self.footer.is_empty() { 0.0 } else { 40.0 }
            + if self.load_more_text.is_empty() {
                0.0
            } else {
                40.0
            };
        Size::new(400.0, h.max(100.0))
    }

    pub fn new() -> Self {
        Self {
            header: String::new(),
            footer: String::new(),
            bordered: true,
            list_size: crate::ui::config::use_config().size,
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
    pub fn size(mut self, s: ControlSize) -> Self {
        self.list_size = s;
        self
    }
    pub fn load_more(mut self, text: impl Into<String>) -> Self {
        self.load_more_text = text.into();
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::List {
            header: self.header.clone(),
            footer: self.footer.clone(),
            bordered: self.bordered,
            list_size: self.list_size,
            items: self.items.clone(),
            load_more_text: self.load_more_text.clone(),
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.header = next.header;
        self.footer = next.footer;
        self.bordered = next.bordered;
        self.list_size = next.list_size;
        self.items = next.items;
        self.load_more_text = next.load_more_text;
    }
}

impl Default for List {
    fn default() -> Self {
        Self::new()
    }
}
