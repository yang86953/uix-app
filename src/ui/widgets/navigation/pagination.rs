//! Pagination widget — 分页器，Ant Design 风格。
//!
//! 支持页码切换、上一页/下一页、快速跳转（省略号）、pageSize 切换。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::Cell;
use std::ops::Range;
use std::rc::Rc;

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
        simple: bool,
        show_jumper: bool,
        total_template: Option<Rc<dyn Fn(usize, Range<usize>) -> String>>,
        size: f32, // item size
        page_size_options: Vec<usize>,
        focused: bool,
        jumper_active: bool,
        jumper_buffer: String,
        jumper_cursor_rect: Cell<Rect>,
        pending_change: Cell<Option<PaginationChange>>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.show_jumper && self.jumper_active }

    text_input_cursor_rect => (&self) -> Rect { self.jumper_cursor_rect.get() }

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
                self.commit_jumper();
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } if self.jumper_active => match key {
                KeyCode::Enter => {
                    self.commit_jumper();
                    EventResult::Handled
                }
                KeyCode::Escape => {
                    self.cancel_jumper();
                    EventResult::Handled
                }
                KeyCode::Backspace => {
                    self.jumper_buffer.pop();
                    EventResult::Handled
                }
                KeyCode::Left
                | KeyCode::Right
                | KeyCode::Home
                | KeyCode::End
                | KeyCode::PageUp
                | KeyCode::PageDown => EventResult::Handled,
                _ => EventResult::NotHandled,
            },
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
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.jumper_active =>
            {
                self.append_jumper_text(text)
            }
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
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
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "chevron-left",
            Rect::new(x, y, item_w, item_h),
            prev_c,
            16.0,
        );
        x += item_w + PAGINATION_GAP;

        if self.simple {
            let simple_rect = Rect::new(x, y, item_w * 2.5, item_h);
            ctx.text_center(
                &format!("{cur} / {total_pages}"),
                simple_rect,
                text,
                13.0,
            );
            x += simple_rect.w + PAGINATION_GAP;
        } else {
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
        }

        let next_disabled = cur >= total_pages || total_pages == 0;
        let next_c = if next_disabled { text_sec } else { text };
        crate::ui::widgets::icon::Icon::paint_in_frame(
            ctx,
            "chevron-right",
            Rect::new(x, y, item_w, item_h),
            next_c,
            16.0,
        );
        x += item_w;

        if self.show_total {
            let total_rect = Rect::new(
                x + PAGINATION_EXTRA_GAP,
                y,
                PAGINATION_TOTAL_WIDTH,
                item_h,
            );
            let total_y = ctx.visual_center_y(total_rect, 12.0);
            let total_label = self
                .total_template
                .as_ref()
                .map(|template| template(self.total, self.current_record_range()))
                .unwrap_or_else(|| format!("共 {} 条", self.total));
            ctx.draw_text(
                &total_label,
                Point::new(total_rect.x, total_y),
                text_sec,
                12.0,
            );
            x = total_rect.x + total_rect.w;
        }

        if self.show_jumper {
            let jumper_rect = Rect::new(x + PAGINATION_EXTRA_GAP, y, 84.0, item_h);
            let label_rect = Rect::new(jumper_rect.x, jumper_rect.y, 24.0, jumper_rect.h);
            let input_rect = Rect::new(
                label_rect.x + label_rect.w,
                jumper_rect.y,
                44.0,
                jumper_rect.h,
            );
            let suffix_rect = Rect::new(
                input_rect.x + input_rect.w + 4.0,
                jumper_rect.y,
                12.0,
                jumper_rect.h,
            );
            let border_color = if self.jumper_active { primary } else { border };
            let label_y = ctx.visual_center_y(label_rect, 12.0);
            ctx.draw_text(
                "跳至",
                Point::new(label_rect.x, label_y),
                text_sec,
                12.0,
            );
            ctx.fill_rect(input_rect, bg, Some(radius));
            ctx.stroke_rect(input_rect, border_color, 1.0, Some(radius));
            let inactive_text = (!self.jumper_active).then(|| cur.to_string());
            let jumper_text = inactive_text
                .as_deref()
                .unwrap_or(self.jumper_buffer.as_str());
            ctx.text_center(jumper_text, input_rect, text, 12.0);
            let suffix_y = ctx.visual_center_y(suffix_rect, 12.0);
            ctx.draw_text(
                "页",
                Point::new(suffix_rect.x, suffix_y),
                text_sec,
                12.0,
            );
            let text_width = ctx.measure_text(jumper_text, 12.0).w.min(input_rect.w - 8.0);
            self.jumper_cursor_rect.set(Rect::new(
                input_rect.x + (input_rect.w + text_width) * 0.5,
                input_rect.y + 5.0,
                1.0,
                (input_rect.h - 10.0).max(0.0),
            ));
            x = jumper_rect.x + jumper_rect.w;
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

        if self.focused && tree.keyboard_focus_visible() {
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
            simple: false,
            show_jumper: false,
            total_template: None,
            size: 28.0,
            page_size_options: vec![10, 20, 50, 100],
            focused: false,
            jumper_active: false,
            jumper_buffer: String::new(),
            jumper_cursor_rect: Cell::new(Rect::zero()),
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

    pub fn simple(mut self, simple: bool) -> Self {
        self.simple = simple;
        self
    }

    pub fn show_jumper(mut self, show: bool) -> Self {
        self.show_jumper = show;
        self
    }

    pub fn total_template<F>(mut self, template: F) -> Self
    where
        F: Fn(usize, std::ops::Range<usize>) -> String + 'static,
    {
        self.total_template = Some(Rc::new(template));
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.total = next.total;
        self.page_size = next.page_size.max(1);
        self.current.set(self.clamp_current(self.current.get()));
        self.show_size_changer = next.show_size_changer;
        self.show_total = next.show_total;
        self.simple = next.simple;
        self.show_jumper = next.show_jumper;
        if !self.show_jumper {
            self.cancel_jumper();
        }
        self.total_template = next.total_template;
        self.size = next.size;
        self.page_size_options = next.page_size_options;
    }

    fn intrinsic_size(&self) -> Size {
        let range_count = self
            .visible_range(self.total_pages(), self.current.get())
            .len() as f32;
        let control_count = if self.simple { 3.0 } else { range_count + 2.0 };
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
        let jumper = if self.show_jumper {
            PAGINATION_EXTRA_GAP + 84.0
        } else {
            0.0
        };
        Size::new(controls + total + changer + jumper, self.size + 8.0)
    }

    pub(crate) fn current_record_range(&self) -> Range<usize> {
        if self.total == 0 {
            return 0..0;
        }
        let start = (self.current.get().saturating_sub(1))
            .saturating_mul(self.page_size)
            .saturating_add(1)
            .min(self.total);
        let end = self
            .current
            .get()
            .saturating_mul(self.page_size)
            .min(self.total);
        start..end
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
        if self.jumper_active
            && self
                .jumper_x_range()
                .is_some_and(|range| !range.contains(&x))
        {
            self.commit_jumper();
        }
        let total_pages = self.total_pages();
        let cur = self.current.get();
        let mut btn_x = 0.0;
        if x >= btn_x && x < btn_x + self.size {
            self.change_page_by(-1);
            return EventResult::Handled;
        }
        btn_x += self.size + PAGINATION_GAP;

        if self.simple {
            if x >= btn_x && x < btn_x + self.size * 2.5 {
                return EventResult::Handled;
            }
            btn_x += self.size * 2.5 + PAGINATION_GAP;
            if x >= btn_x && x < btn_x + self.size {
                self.change_page_by(1);
                return EventResult::Handled;
            }
            return EventResult::NotHandled;
        }

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
        if self.show_jumper {
            let jumper_x = btn_x + PAGINATION_EXTRA_GAP;
            if x >= jumper_x && x < jumper_x + 84.0 {
                self.begin_jumper_edit();
                return EventResult::Handled;
            }
            if self.jumper_active {
                self.commit_jumper();
            }
            btn_x = jumper_x + 84.0;
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

    pub(crate) fn jumper_x_range(&self) -> Option<std::ops::Range<f32>> {
        if !self.show_jumper {
            return None;
        }
        let range_count = self
            .visible_range(self.total_pages(), self.current.get())
            .len() as f32;
        let control_count = if self.simple { 3.0 } else { range_count + 2.0 };
        let controls = control_count * self.size + (control_count - 1.0) * PAGINATION_GAP;
        let total = if self.show_total {
            PAGINATION_EXTRA_GAP + PAGINATION_TOTAL_WIDTH
        } else {
            0.0
        };
        let start = controls + total + PAGINATION_EXTRA_GAP;
        Some(start..start + 84.0)
    }

    fn begin_jumper_edit(&mut self) {
        self.jumper_active = true;
        self.jumper_buffer.clear();
    }

    fn append_jumper_text(&mut self, text: &str) -> EventResult {
        let before = self.jumper_buffer.len();
        let max_digits = self.total_pages().max(1).to_string().len().max(1);
        self.jumper_buffer.extend(
            text.chars()
                .filter(char::is_ascii_digit)
                .take(max_digits.saturating_sub(self.jumper_buffer.len())),
        );
        if self.jumper_buffer.len() != before {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn commit_jumper(&mut self) {
        if !self.jumper_active {
            return;
        }
        if let Ok(page) = self.jumper_buffer.parse::<usize>() {
            self.select_page(page);
        }
        self.jumper_active = false;
        self.jumper_buffer.clear();
    }

    fn cancel_jumper(&mut self) {
        self.jumper_active = false;
        self.jumper_buffer.clear();
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
