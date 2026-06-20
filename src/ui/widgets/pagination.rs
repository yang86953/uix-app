//! Pagination widget — 分页器，Ant Design 风格。
//!
//! 支持页码切换、上一页/下一页、快速跳转（省略号）、pageSize 切换。

use crate::base::{Point, Rect, Size};
use crate::define_widget;
use crate::graphics::{Color, Radius};
use crate::ui::render_context::RenderContext;
use crate::ui::widget::{EventResult, WidgetEvent, WidgetTree};
use std::cell::Cell;

/// Pagination — 分页器。
define_widget! {
    pub struct Pagination {
        total: usize,
        page_size: usize,
        current: Cell<usize>,
        show_size_changer: bool,
        show_total: bool,
        size: f32, // item size
    }

    preferred_size => (&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        let pages = self.total.div_ceil(self.page_size);
        let count = pages.min(7) as f32; // 最多显示 7 个页码按钮
        Size::new(count * (self.size + 4.0) + 80.0, self.size + 8.0)
    }

    on_event => (&mut self, event: &WidgetEvent) -> EventResult {
        if let WidgetEvent::MouseDown { pos, .. } = event {
            let total_pages = self.total.div_ceil(self.page_size);
            let mut cur = self.current.get();
            let item_w = self.size;
            let gap = 4.0;
            let mut x = pos.x;
            // 上一页
            let mut btn_x = 0.0;
            if x >= btn_x && x < btn_x + item_w { cur = cur.saturating_sub(1).max(1); self.current.set(cur); return EventResult::Handled; }
            btn_x += item_w + gap;
            // 页码按钮（最多 7 个）
            let range = self.visible_range(total_pages, cur);
            for &p in &range {
                if x >= btn_x && x < btn_x + item_w {
                    self.current.set(p);
                    return EventResult::Handled;
                }
                btn_x += item_w + gap;
            }
            // 下一页
            if x >= btn_x && x < btn_x + item_w { cur = (cur + 1).min(total_pages); self.current.set(cur); return EventResult::Handled; }
        }
        EventResult::NotHandled
    }

    render => (&self, frame: Rect, ctx: &mut RenderContext, _tree: &WidgetTree) {
        let total_pages = self.total.div_ceil(self.page_size);
        if total_pages <= 1 { return; }
        let cur = self.current.get();
        let item_w = self.size;
        let item_h = self.size;
        let gap = 4.0;
        let radius = Radius::uniform(ctx.tokens().border_radius_sm());
        let primary = ctx.tokens().color_primary();
        let border = ctx.tokens().color_border();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let white = Color::white();
        let bg = ctx.tokens().color_bg_container();

        let mut x = frame.x;
        let y = frame.y + (frame.h - item_h) * 0.5;
        let range = self.visible_range(total_pages, cur);
        // 上一页
        let prev_disabled = cur <= 1;
        let prev_c = if prev_disabled { text_sec } else { text };
        let prev_y = ctx.visual_center_y(Rect::new(x, y, item_w, item_h), 16.0);
        ctx.draw_text("‹", Point::new(x + item_w * 0.5 - 5.0, prev_y), prev_c, 16.0);
        x += item_w + gap;

        // 页码按钮
        for &p in &range {
            let active = p == cur;
            let btn_rect = Rect::new(x, y, item_w, item_h);
            if active {
                ctx.fill_rect(btn_rect, primary, Some(radius));
                let py = ctx.visual_center_y(btn_rect, 13.0);
                ctx.draw_text(&p.to_string(), Point::new(x + item_w * 0.5 - 5.0, py), white, 13.0);
            } else {
                ctx.fill_rect(btn_rect, bg, Some(radius));
                ctx.stroke_rect(btn_rect, border, 1.0, Some(radius));
                let py = ctx.visual_center_y(btn_rect, 13.0);
                ctx.draw_text(&p.to_string(), Point::new(x + item_w * 0.5 - 5.0, py), text, 13.0);
            }
            x += item_w + gap;
        }

        // 下一页
        let next_disabled = cur >= total_pages;
        let next_c = if next_disabled { text_sec } else { text };
        let next_y = ctx.visual_center_y(Rect::new(x, y, item_w, item_h), 16.0);
        ctx.draw_text("›", Point::new(x + item_w * 0.5 - 5.0, next_y), next_c, 16.0);

        // 总条数（右侧）
        if self.show_total {
            let total_rect = Rect::new(x + item_w + 12.0, y, 160.0, item_h);
            let total_y = ctx.visual_center_y(total_rect, 12.0);
            ctx.draw_text(&format!("共 {} 条", self.total), Point::new(x + item_w + 12.0, total_y), text_sec, 12.0);
        }
    }
}

impl Pagination {
    pub fn new(total: usize, page_size: usize) -> Self {
        Self {
            total, page_size,
            current: Cell::new(1),
            show_size_changer: false,
            show_total: true,
            size: 28.0,
        }
    }
    pub fn current(mut self, v: usize) -> Self { self.current.set(v); self }
    pub fn get_current(&self) -> usize { self.current.get() }
    pub fn page_size(mut self, v: usize) -> Self { self.page_size = v; self }
    pub fn show_total(mut self, v: bool) -> Self { self.show_total = v; self }
    pub fn item_size(mut self, v: f32) -> Self { self.size = v; self }
    pub fn set_current(&self, v: usize) { self.current.set(v); }
    pub fn total_pages(&self) -> usize { self.total.div_ceil(self.page_size) }

    /// 计算可见页码范围（含省略号逻辑，最多 7 个按钮）。
    fn visible_range(&self, total_pages: usize, cur: usize) -> Vec<usize> {
        if total_pages <= 7 {
            return (1..=total_pages).collect();
        }
        let mut pages = Vec::new();
        pages.push(1);
        if cur > 3 { pages.push(0); } // 省略号用 0 表示
        let start = (cur.saturating_sub(1)).max(2);
        let end = (cur + 1).min(total_pages - 1);
        for p in start..=end { pages.push(p); }
        if cur < total_pages - 2 { pages.push(0); }
        pages.push(total_pages);
        pages
    }
}
