//! Descriptions widget — 描述列表，Ant Design 风格。
//!
//! 用于只读展示多条字段信息，支持 bordered、column 布局、label/value 键值对。

use crate::core::{Point, Rect, Size};
use crate::define_widget;
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::native::traits::input::ControlSize;
use crate::ui::WidgetTree;

/// 单个描述项。
#[derive(Debug, Clone)]
pub struct DescriptionsItem {
    pub label: String,
    pub value: String,
    pub span: usize,
}

// Descriptions — 描述列表。
define_widget! {
    pub struct Descriptions {
        title: String,
        items: Vec<DescriptionsItem>,
        bordered: bool,
        column: usize,
        label_width: f32,
        size: ControlSize,
    }

    preferred_size => (&self, _engine: Option<&dyn crate::draw::traits::GraphicsEngine>) -> Size {
        let rows = self.items.len().div_ceil(self.column).max(1);
        let title_h = if self.title.is_empty() { 0.0 } else { 32.0 };
        let item_h = match self.size {
            ControlSize::Small => 28.0,
            ControlSize::Medium => 36.0,
            ControlSize::Large => 44.0,
        };
        Size::new(600.0, title_h + rows as f32 * item_h)
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_quaternary();
        let r = Radius::uniform(ctx.tokens().border_radius());
        let mut y = frame.y;
        let col_w = frame.w / self.column as f32;
        let item_h = match self.size {
            ControlSize::Small => 28.0,
            ControlSize::Medium => 36.0,
            ControlSize::Large => 44.0,
        };

        // 标题
        if !self.title.is_empty() {
            let title_rect = Rect::new(frame.x, y, frame.w, 32.0);
            let ty = ctx.visual_center_y(title_rect, 15.0);
            ctx.draw_text(&self.title, Point::new(frame.x + 12.0, ty), text, 15.0);
            y += 32.0;
        }

        // 主体背景
        if self.bordered {
            ctx.fill_rect(Rect::new(frame.x, y, frame.w, self.items.len().div_ceil(self.column) as f32 * item_h), bg, Some(r));
            ctx.stroke_rect(Rect::new(frame.x, y, frame.w, self.items.len().div_ceil(self.column) as f32 * item_h), border, 1.0, Some(r));
        }

        // 逐行渲染
        for (i, item) in self.items.iter().enumerate() {
            let col = i % self.column;
            let row = i / self.column;
            let item_x = frame.x + col as f32 * col_w;
            let item_y = y + row as f32 * item_h;
            let item_w = item.span as f32 * col_w;

            let row_rect = Rect::new(item_x, item_y, item_w, item_h);
            let row_y = ctx.visual_center_y(row_rect, 13.0);
            if self.bordered {
                ctx.fill_rect(Rect::new(item_x, item_y, self.label_width, item_h), fill, None);
                ctx.stroke_rect(Rect::new(item_x, item_y, item_w, item_h), border, 1.0, None);
                ctx.draw_text(&item.label, Point::new(item_x + 8.0, row_y), text_sec, 13.0);
                ctx.draw_text(&item.value, Point::new(item_x + self.label_width + 8.0, row_y), text, 13.0);
            } else {
                ctx.draw_text(&item.label, Point::new(item_x + 8.0, row_y), text_sec, 13.0);
                let val_x = item_x + self.label_width;
                ctx.draw_text(&item.value, Point::new(val_x, row_y), text, 13.0);
            }
        }
    }
}

impl Descriptions {
    pub fn new() -> Self {
        Self {
            title: String::new(),
            items: Vec::new(),
            bordered: false,
            column: 3,
            label_width: 100.0,
            size: ControlSize::Medium,
        }
    }
    pub fn title(mut self, t: &str) -> Self {
        self.title = t.to_string();
        self
    }
    pub fn items(mut self, items: Vec<DescriptionsItem>) -> Self {
        self.items = items;
        self
    }
    pub fn add(mut self, item: DescriptionsItem) -> Self {
        self.items.push(item);
        self
    }
    pub fn bordered(mut self, v: bool) -> Self {
        self.bordered = v;
        self
    }
    pub fn column(mut self, v: usize) -> Self {
        self.column = v;
        self
    }
    pub fn label_width(mut self, w: f32) -> Self {
        self.label_width = w;
        self
    }
    pub fn size(mut self, s: ControlSize) -> Self {
        self.size = s;
        self
    }
}

impl Default for Descriptions {
    fn default() -> Self {
        Self::new()
    }
}

impl DescriptionsItem {
    pub fn new(label: &str, value: &str) -> Self {
        Self {
            label: label.to_string(),
            value: value.to_string(),
            span: 1,
        }
    }
    pub fn span(mut self, s: usize) -> Self {
        self.span = s;
        self
    }
}
