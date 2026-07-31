//! List widget — 列表组件，Ant Design 风格。
//!
//! 支持列表项渲染、header/footer、bordered、size 等选项。

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::Radius;
use crate::native::windowing::input::ControlSize;
use crate::ui::component::paint_context::PaintContext;
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

    picture_policy => (&self) -> crate::draw::scene::PicturePolicy {
        crate::draw::scene::PicturePolicy::Eligible
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let bg = ctx.tokens().color_bg_container();
        let border = ctx.tokens().color_border_secondary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let item_h = list_item_height(self.list_size);
        let radius = ctx.tokens().border_radius().min(frame.w.min(frame.h) * 0.5);
        let r = Radius::uniform(radius);
        let mut y = frame.y;
        let horizontal_padding = 16.0_f32.min(frame.w * 0.25);

        ctx.push_clip(frame);
        ctx.fill_rect(frame, bg, Some(r));
        if self.bordered {
            let border_frame = Self::inset(frame, 0.5);
            ctx.stroke_rect(
                border_frame,
                border,
                1.0,
                Some(Radius::uniform(
                    radius.min(border_frame.w.min(border_frame.h) * 0.5),
                )),
            );
        }

        if !self.header.is_empty() {
            let header_rect = Self::row_content_rect(frame, y, item_h, horizontal_padding);
            Self::paint_single_line(ctx, &self.header, header_rect, text_sec, 13.0);
            ctx.fill_rect(Rect::new(frame.x, y + item_h, frame.w, 1.0), border, None);
            y += item_h;
        }

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Self::row_content_rect(frame, y, item_h, horizontal_padding);
            Self::paint_single_line(ctx, item, item_rect, text, 14.0);
            if i < self.items.len() - 1 {
                ctx.fill_rect(
                    Rect::new(
                        frame.x + horizontal_padding,
                        y + item_h - 1.0,
                        (frame.w - horizontal_padding * 2.0).max(0.0),
                        1.0,
                    ),
                    border,
                    None,
                );
            }
            y += item_h;
        }

        if !self.footer.is_empty() {
            if !self.items.is_empty() {
                ctx.fill_rect(Rect::new(frame.x, y, frame.w, 1.0), border, None);
            }
            let footer_rect = Self::row_content_rect(frame, y, item_h, horizontal_padding);
            Self::paint_single_line(ctx, &self.footer, footer_rect, text_sec, 13.0);
            y += item_h;
        }

        if !self.load_more_text.is_empty() {
            let load_rect = Rect::new(frame.x, y, frame.w, 40.0);
            ctx.fill_rect(load_rect, bg, None);
            ctx.stroke_rect(load_rect, border, 1.0, Some(r));
            let load_content = Self::row_content_rect(frame, y, 40.0, horizontal_padding);
            let primary = ctx.tokens().color_primary();
            Self::paint_single_line(
                ctx,
                &self.load_more_text,
                load_content,
                primary,
                14.0,
            );
        }
        ctx.pop_clip();
    }
}

impl List {
    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn inset(frame: Rect, amount: f32) -> Rect {
        let amount = amount.min(frame.w * 0.5).min(frame.h * 0.5).max(0.0);
        Rect::new(
            frame.x + amount,
            frame.y + amount,
            (frame.w - amount * 2.0).max(0.0),
            (frame.h - amount * 2.0).max(0.0),
        )
    }

    fn row_content_rect(frame: Rect, y: f32, height: f32, horizontal_padding: f32) -> Rect {
        Rect::new(
            frame.x + horizontal_padding,
            y,
            (frame.w - horizontal_padding * 2.0).max(0.0),
            height.max(0.0),
        )
    }

    fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size);
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }

    fn intrinsic_size(&self) -> Size {
        let item_h = list_item_height(self.list_size);
        let h = self.items.len() as f32 * item_h
            + if self.header.is_empty() { 0.0 } else { item_h }
            + if self.footer.is_empty() { 0.0 } else { item_h }
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
            list_size: crate::ui::component::config::use_config().size,
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

impl crate::ui::view::View for List {
    fn build(self) -> crate::ui::view::ViewNode {
        if self.items.is_empty() {
            if let Some(empty) = crate::ui::component::config::render_empty_for::<Self>() {
                return empty;
            }
            return crate::ui::view::ViewNode::leaf(crate::ui::widgets::display::Empty::new());
        }
        crate::ui::view::ViewNode::leaf(self)
    }
}
