//! Pagination widget — 分页器，Ant Design 风格。
//!
//! 支持页码切换、上一页/下一页、快速跳转（省略号）、pageSize 切换。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;

const PAGINATION_GAP: f32 = 4.0;
const PAGINATION_EXTRA_GAP: f32 = 12.0;
const PAGINATION_TOTAL_WIDTH: f32 = 100.0;
const PAGINATION_SIZE_WIDTH: f32 = 80.0;

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
        focused: bool,
        pending_change: Cell<Option<PaginationChange>>,
    }

    tab_index => (&self) -> i32 { 1 }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if pos.y < 0.0 || pos.y > self.size + 8.0 {
                    return EventResult::NotHandled;
                }
                self.handle_pointer_down(pos.x)
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Left | KeyCode::PageUp => {
                    self.change_page_by(-1);
                    EventResult::Handled
                }
                KeyCode::Right | KeyCode::PageDown => {
                    self.change_page_by(1);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select_page(1);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select_page(self.total_pages().max(1));
                    EventResult::Handled
                }
                KeyCode::Up if self.show_size_changer => {
                    self.cycle_page_size(false);
                    EventResult::Handled
                }
                KeyCode::Down if self.show_size_changer => {
                    self.cycle_page_size(true);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change.take().map(|change| match change {
            PaginationChange::Page(page) => SemanticEvent::change(id, page.to_string()),
            PaginationChange::PageSize(page_size) => {
                SemanticEvent::change(id, format!("page_size={page_size}"))
            }
        })
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let loc = crate::ui::locale::use_locale();
        let total_pages = self.total.div_ceil(self.page_size);
        if total_pages == 0 && !self.show_total && !self.show_size_changer {
            return;
        }
        let cur = self.current.get();
        let item_w = self.size;
        let item_h = self.size;
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
        let prev_disabled = cur <= 1 || total_pages == 0;
        let prev_c = if prev_disabled { text_sec } else { text };
        let prev_y = ctx.visual_center_y(Rect::new(x, y, item_w, item_h), 16.0);
        ctx.draw_text(loc.pagination_prev_symbol, Point::new(x + item_w * 0.5 - 5.0, prev_y), prev_c, 16.0);
        x += item_w + PAGINATION_GAP;

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
            x += item_w + PAGINATION_GAP;
        }

        let next_disabled = cur >= total_pages || total_pages == 0;
        let next_c = if next_disabled { text_sec } else { text };
        let next_y = ctx.visual_center_y(Rect::new(x, y, item_w, item_h), 16.0);
        ctx.draw_text(loc.pagination_next_symbol, Point::new(x + item_w * 0.5 - 5.0, next_y), next_c, 16.0);
        x += item_w;

        if self.show_total {
            let total_rect = Rect::new(
                x + PAGINATION_EXTRA_GAP,
                y,
                PAGINATION_TOTAL_WIDTH,
                item_h,
            );
            let total_y = ctx.visual_center_y(total_rect, 12.0);
            ctx.draw_text(
                &format!("共 {} 条", self.total),
                Point::new(total_rect.x, total_y),
                text_sec,
                12.0,
            );
            x = total_rect.x + total_rect.w;
        }

        if self.show_size_changer && !self.page_size_options.is_empty() {
            let changer_text = format!("{} 条/页", self.page_size);
            let changer_rect = Rect::new(
                x + PAGINATION_EXTRA_GAP,
                y,
                PAGINATION_SIZE_WIDTH,
                item_h,
            );
            ctx.stroke_rect(changer_rect, border, 1.0, Some(radius));
            let cy = ctx.visual_center_y(changer_rect, 12.0);
            ctx.draw_text(
                &changer_text,
                Point::new(changer_rect.x + 6.0, cy),
                text,
                12.0,
            );
        }

        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, Some(radius));
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PaginationChange {
    Page(usize),
    PageSize(usize),
}

impl Pagination {
    pub fn new(total: usize, page_size: usize) -> Self {
        Self {
            total,
            page_size: page_size.max(1),
            current: Cell::new(1),
            show_size_changer: false,
            show_total: true,
            size: 28.0,
            page_size_options: vec![10, 20, 50, 100],
            focused: false,
            pending_change: Cell::new(None),
        }
    }
    pub fn current(self, v: usize) -> Self {
        self.current.set(self.clamp_current(v));
        self
    }
    pub fn get_current(&self) -> usize {
        self.current.get()
    }
    pub fn get_page_size(&self) -> usize {
        self.page_size
    }
    pub fn page_size(mut self, v: usize) -> Self {
        self.page_size = v.max(1);
        self.current.set(self.clamp_current(self.current.get()));
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
        self.current.set(self.clamp_current(v));
    }
    pub fn total_pages(&self) -> usize {
        self.total.div_ceil(self.page_size)
    }
    pub fn show_size_changer(mut self, v: bool) -> Self {
        self.show_size_changer = v;
        self
    }
    pub fn page_size_options(mut self, opts: Vec<usize>) -> Self {
        self.page_size_options.clear();
        for option in opts.into_iter().map(|option| option.max(1)) {
            if !self.page_size_options.contains(&option) {
                self.page_size_options.push(option);
            }
        }
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.total = next.total;
        self.page_size = next.page_size.max(1);
        self.current.set(self.clamp_current(self.current.get()));
        self.show_size_changer = next.show_size_changer;
        self.show_total = next.show_total;
        self.size = next.size;
        self.page_size_options = next.page_size_options;
    }

    fn intrinsic_size(&self) -> Size {
        let range_count = self
            .visible_range(self.total_pages(), self.current.get())
            .len() as f32;
        let control_count = range_count + 2.0;
        let controls = control_count * self.size + (control_count - 1.0) * PAGINATION_GAP;
        let total = if self.show_total {
            PAGINATION_EXTRA_GAP + PAGINATION_TOTAL_WIDTH
        } else {
            0.0
        };
        let changer = if self.show_size_changer && !self.page_size_options.is_empty() {
            PAGINATION_EXTRA_GAP + PAGINATION_SIZE_WIDTH
        } else {
            0.0
        };
        Size::new(controls + total + changer, self.size + 8.0)
    }

    /// 计算可见页码范围（含省略号逻辑，最多 7 个按钮）。
    pub(crate) fn visible_range(&self, total_pages: usize, cur: usize) -> Vec<usize> {
        if total_pages == 0 {
            return Vec::new();
        }
        if total_pages <= 7 {
            return (1..=total_pages).collect();
        }
        let mut pages = Vec::new();
        pages.push(1);
        if cur > 3 {
            pages.push(0);
        } // 省略号用 0 表示
        let start = (cur.saturating_sub(1)).max(2);
        let end = cur.saturating_add(1).min(total_pages - 1);
        for p in start..=end {
            pages.push(p);
        }
        if cur < total_pages - 2 {
            pages.push(0);
        }
        pages.push(total_pages);
        pages
    }

    fn clamp_current(&self, current: usize) -> usize {
        let total_pages = self.total_pages();
        if total_pages == 0 {
            1
        } else {
            current.clamp(1, total_pages)
        }
    }

    fn handle_pointer_down(&mut self, x: f32) -> EventResult {
        let total_pages = self.total_pages();
        let cur = self.current.get();
        let mut btn_x = 0.0;
        if x >= btn_x && x < btn_x + self.size {
            self.change_page_by(-1);
            return EventResult::Handled;
        }
        btn_x += self.size + PAGINATION_GAP;

        for page in self.visible_range(total_pages, cur) {
            if page > 0 && x >= btn_x && x < btn_x + self.size {
                self.select_page(page);
                return EventResult::Handled;
            }
            btn_x += self.size + PAGINATION_GAP;
        }

        if x >= btn_x && x < btn_x + self.size {
            self.change_page_by(1);
            return EventResult::Handled;
        }
        btn_x += self.size;

        if self.show_total {
            btn_x += PAGINATION_EXTRA_GAP + PAGINATION_TOTAL_WIDTH;
        }
        if self.show_size_changer && !self.page_size_options.is_empty() {
            let changer_x = btn_x + PAGINATION_EXTRA_GAP;
            if x >= changer_x && x < changer_x + PAGINATION_SIZE_WIDTH {
                self.cycle_page_size(true);
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    fn change_page_by(&self, delta: isize) {
        let current = self.current.get();
        let next = if delta < 0 {
            current.saturating_sub(delta.unsigned_abs()).max(1)
        } else {
            current
                .saturating_add(delta as usize)
                .min(self.total_pages().max(1))
        };
        self.select_page(next);
    }

    fn select_page(&self, page: usize) {
        let page = self.clamp_current(page);
        if page != self.current.get() {
            self.current.set(page);
            self.pending_change.set(Some(PaginationChange::Page(page)));
        }
    }

    fn cycle_page_size(&mut self, forward: bool) {
        if self.page_size_options.is_empty() {
            return;
        }
        let current = self
            .page_size_options
            .iter()
            .position(|option| *option == self.page_size);
        let next = match (current, forward) {
            (Some(index), true) => (index + 1) % self.page_size_options.len(),
            (Some(index), false) => {
                (index + self.page_size_options.len() - 1) % self.page_size_options.len()
            }
            (None, true) => 0,
            (None, false) => self.page_size_options.len() - 1,
        };
        let page_size = self.page_size_options[next];
        if page_size != self.page_size {
            self.page_size = page_size;
            self.current.set(self.clamp_current(self.current.get()));
            self.pending_change
                .set(Some(PaginationChange::PageSize(page_size)));
        }
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Pagination {
            total: self.total,
            page_size: self.page_size,
            current: self.current.get(),
            show_size_changer: self.show_size_changer,
            show_total: self.show_total,
            size: self.size,
            page_size_options: self.page_size_options.clone(),
        }
    }
}
