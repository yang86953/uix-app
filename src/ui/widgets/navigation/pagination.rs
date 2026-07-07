//! Pagination widget — 分页器，Ant Design 风格。
//!
//! 支持页码切换、上一页/下一页、快速跳转（省略号）、pageSize 切换。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::SnapshotFields;
use crate::ui::{EventResult, SemanticEvent, SystemEvent, WidgetId, WidgetTree};
use std::cell::Cell;

// Pagination — 分页器。
component! {
    pub struct Pagination {
        total: usize,
        page_size: usize,
        current: Cell<usize>,
        show_size_changer: bool,
        show_total: bool,
        size: f32, // item size
        page_size_options: Vec<usize>,
        pending_change: Cell<Option<usize>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        if let SystemEvent::PointerDown { pos, .. } = event {
            let total_pages = self.total.div_ceil(self.page_size);
            let cur = self.current.get();
            let item_w = self.size;
            let gap = 4.0;
            let x = pos.x;
            // 上一页
            let mut btn_x = 0.0;
            if x >= btn_x && x < btn_x + item_w {
                let next = cur.saturating_sub(1).max(1);
                if next != cur {
                    self.current.set(next);
                    self.pending_change.set(Some(next));
                }
                return EventResult::Handled;
            }
            btn_x += item_w + gap;
            // 页码按钮（最多 7 个，p=0 表示省略号，跳过点击）
            let range = self.visible_range(total_pages, cur);
            for &p in &range {
                if p > 0 && x >= btn_x && x < btn_x + item_w {
                    if p != cur {
                        self.current.set(p);
                        self.pending_change.set(Some(p));
                    }
                    return EventResult::Handled;
                }
                btn_x += item_w + gap;
            }
            // 下一页
            if x >= btn_x && x < btn_x + item_w {
                let next = (cur + 1).min(total_pages);
                if next != cur {
                    self.current.set(next);
                    self.pending_change.set(Some(next));
                }
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .take()
            .map(|page| SemanticEvent::change(id, page.to_string()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
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
        ctx.draw_text(loc.pagination_prev_symbol, Point::new(x + item_w * 0.5 - 5.0, prev_y), prev_c, 16.0);
        x += item_w + gap;

        // 页码按钮（0 表示省略号，显示为 "..."）
        for &p in &range {
            let label = if p == 0 { "...".to_string() } else { p.to_string() };
            let active = p == cur;
            let btn_rect = Rect::new(x, y, item_w, item_h);
            if active {
                ctx.fill_rect(btn_rect, primary, Some(radius));
                ctx.text_center(&label, btn_rect, white, 13.0);
            } else {
                ctx.fill_rect(btn_rect, bg, Some(radius));
                ctx.stroke_rect(btn_rect, border, 1.0, Some(radius));
                ctx.text_center(&label, btn_rect, text, 13.0);
            }
            x += item_w + gap;
        }

        // 下一页
        let next_disabled = cur >= total_pages;
        let next_c = if next_disabled { text_sec } else { text };
        let next_y = ctx.visual_center_y(Rect::new(x, y, item_w, item_h), 16.0);
        ctx.draw_text(loc.pagination_next_symbol, Point::new(x + item_w * 0.5 - 5.0, next_y), next_c, 16.0);

        // 总条数（右侧）
        if self.show_total {
            let total_rect = Rect::new(x + item_w + 12.0, y, 160.0, item_h);
            let total_y = ctx.visual_center_y(total_rect, 12.0);
            ctx.draw_text(&format!("共 {} 条", self.total), Point::new(x + item_w + 12.0, total_y), text_sec, 12.0);
        }

        // 尺寸切换
        if self.show_size_changer && !self.page_size_options.is_empty() {
            let changer_x = x + item_w + if self.show_total { 120.0 } else { 12.0 };
            let changer_text = format!("{} 条/页", self.page_size);
            let changer_rect = Rect::new(changer_x, y, 80.0, item_h);
            ctx.stroke_rect(changer_rect, border, 1.0, Some(radius));
            let cy = ctx.visual_center_y(changer_rect, 12.0);
            ctx.draw_text(&changer_text, Point::new(changer_x + 6.0, cy), text, 12.0);
        }
    }
}

impl Pagination {
    pub fn new(total: usize, page_size: usize) -> Self {
        Self {
            total,
            page_size,
            current: Cell::new(1),
            show_size_changer: false,
            show_total: true,
            size: 28.0,
            page_size_options: Vec::new(),
            pending_change: Cell::new(None),
        }
    }
    pub fn current(self, v: usize) -> Self {
        self.current.set(v);
        self
    }
    pub fn get_current(&self) -> usize {
        self.current.get()
    }
    pub fn page_size(mut self, v: usize) -> Self {
        self.page_size = v;
        self
    }
    pub fn show_total(mut self, v: bool) -> Self {
        self.show_total = v;
        self
    }
    pub fn item_size(mut self, v: f32) -> Self {
        self.size = v;
        self
    }
    pub fn set_current(&self, v: usize) {
        self.current.set(v);
    }
    pub fn total_pages(&self) -> usize {
        self.total.div_ceil(self.page_size)
    }
    pub fn show_size_changer(mut self, v: bool) -> Self {
        self.show_size_changer = v;
        self
    }
    pub fn page_size_options(mut self, opts: Vec<usize>) -> Self {
        self.page_size_options = opts;
        self
    }

    fn intrinsic_size(&self) -> Size {
        let pages = self.total.div_ceil(self.page_size);
        let count = pages.min(7) as f32; // 最多显示 7 个页码按钮
        Size::new(count * (self.size + 4.0) + 80.0, self.size + 8.0)
    }

    /// 计算可见页码范围（含省略号逻辑，最多 7 个按钮）。
    fn visible_range(&self, total_pages: usize, cur: usize) -> Vec<usize> {
        if total_pages <= 7 {
            return (1..=total_pages).collect();
        }
        let mut pages = Vec::new();
        pages.push(1);
        if cur > 3 {
            pages.push(0);
        } // 省略号用 0 表示
        let start = (cur.saturating_sub(1)).max(2);
        let end = (cur + 1).min(total_pages - 1);
        for p in start..=end {
            pages.push(p);
        }
        if cur < total_pages - 2 {
            pages.push(0);
        }
        pages.push(total_pages);
        pages
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Pagination {
            total: self.total,
            page_size: self.page_size,
            show_size_changer: self.show_size_changer,
            show_total: self.show_total,
            size: self.size,
            page_size_options: self.page_size_options.clone(),
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/navigation/pagination.rs"]
mod tests;
