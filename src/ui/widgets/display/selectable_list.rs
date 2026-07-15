//! Selectable list widget.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SelectableListAction {
    Header,
    Row(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub struct SelectableItem {
    pub id: String,
    pub text: String,
    pub icon: Option<String>,
}

impl Default for SelectableList {
    fn default() -> Self {
        Self::new()
    }
}

impl SelectableList {
    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::SelectableList {
            items: self.items.clone(),
            active_index: self.active_index,
            header_button_text: self.header_button_text.clone(),
            footer_text: self.footer_text.clone(),
            item_height: self.item_height,
        }
    }

    pub(crate) fn item_stride(&self) -> f32 {
        self.item_height + 2.0
    }

    fn list_body_top(&self) -> f32 {
        if self.header_button_text.is_empty() {
            0.0
        } else {
            48.0
        }
    }

    pub(crate) fn list_body_viewport_height(&self) -> f32 {
        let frame = self
            .last_frame
            .get()
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 220.0, 500.0));
        let footer = if self.footer_text.is_empty() {
            0.0
        } else {
            28.0
        };
        (frame.h - self.list_body_top() - footer).max(self.item_stride())
    }

    pub(crate) fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let list_top = self.list_body_top();
        let viewport_height = self.list_body_viewport_height();
        if pos_y < list_top || pos_y >= list_top + viewport_height {
            return None;
        }
        let local_y = pos_y - list_top + self.body_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let row = (local_y / self.item_stride()) as usize;
        if row < self.items.len() {
            Some(row)
        } else {
            None
        }
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    fn header_button_rect(&self) -> Rect {
        let width = self.last_frame.get().map_or(220.0, |frame| frame.w);
        Rect::new(8.0, 8.0, (width - 16.0).max(0.0), 32.0)
    }

    fn select(&mut self, index: usize, activate_unchanged: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index;
        self.active_index = index;
        if changed || activate_unchanged {
            self.pending_action
                .set(Some(SelectableListAction::Row(index)));
        }
        if changed {
            self.ensure_active_visible();
        }
    }

    fn move_active(&mut self, forward: bool) {
        if self.items.is_empty() {
            return;
        }
        let next = if forward {
            (self.active_index + 1).min(self.items.len() - 1)
        } else {
            self.active_index.saturating_sub(1)
        };
        self.select(next, false);
    }

    fn ensure_active_visible(&mut self) {
        let stride = self.item_stride();
        let viewport_height = self.list_body_viewport_height();
        let old_offset = self.body_scroll.scroll_offset();
        let row_top = self.active_index as f32 * stride;
        let row_bottom = row_top + stride;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.body_scroll.set_scroll_offset(new_offset);
        self.body_scroll
            .clamp_to_content(self.items.len(), stride, viewport_height);
        self.push_scroll_delta(0.0, self.body_scroll.scroll_offset() - old_offset);
    }

    pub fn items(mut self, items: Vec<SelectableItem>) -> Self {
        self.items = items;
        self.active_index = self.active_index.min(self.items.len().saturating_sub(1));
        self
    }

    pub fn active(mut self, index: usize) -> Self {
        self.active_index = index.min(self.items.len().saturating_sub(1));
        self
    }

    pub fn header_button(mut self, text: impl Into<String>) -> Self {
        self.header_button_text = text.into();
        self
    }

    pub fn footer(mut self, text: impl Into<String>) -> Self {
        self.footer_text = text.into();
        self
    }

    pub fn row_height(mut self, height: f32) -> Self {
        if height.is_finite() {
            self.item_height = height.max(20.0);
        }
        self
    }

    pub fn selected_id(&self) -> Option<&str> {
        self.items
            .get(self.active_index)
            .map(|item| item.id.as_str())
    }

    pub fn selected_text(&self) -> Option<&str> {
        self.items
            .get(self.active_index)
            .map(|item| item.text.as_str())
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.items = next.items;
        self.header_button_text = next.header_button_text;
        self.footer_text = next.footer_text;
        self.item_height = next.item_height;
        if self.items.is_empty() {
            self.active_index = 0;
        } else {
            self.active_index = self.active_index.min(self.items.len() - 1);
        }
        self.body_scroll.clamp_to_content(
            self.items.len(),
            self.item_stride(),
            self.list_body_viewport_height(),
        );
        self.hovered_index.set(
            self.hovered_index
                .get()
                .filter(|idx| *idx < self.items.len()),
        );
    }
}

impl SelectableItem {
    pub fn new(id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            text: text.into(),
            icon: None,
        }
    }

    pub fn icon(mut self, icon: &str) -> Self {
        self.icon = Some(icon.to_string());
        self
    }
}

component! {
    /// A vertical list with selectable rows.
    pub struct SelectableList {
        pub items: Vec<SelectableItem>,
        pub active_index: usize,
        pub header_button_text: String,
        pub footer_text: String,
        pub item_height: f32,

        hovered_index: Cell<Option<usize>>,
        hovered_header: Cell<bool>,
        focused: bool,
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        pending_action: Cell<Option<SelectableListAction>>,
    }

    @new -> Self {
        Self {
            items: Vec::new(),
            active_index: 0,
            header_button_text: String::new(),
            footer_text: String::new(),
            item_height: 36.0,
            hovered_index: Cell::new(None),
            hovered_header: Cell::new(false),
            focused: false,
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            pending_action: Cell::new(None),
        }
    }

    tab_index => (&self) -> i32 {
        i32::from(!self.items.is_empty() || !self.header_button_text.is_empty())
    }

    measure => (&self, constraints: Constraints) -> Size {
        let mut h = 0.0;
        if !self.header_button_text.is_empty() {
            h += 48.0;
        }
        h += self.items.len() as f32 * self.item_stride();
        if !self.footer_text.is_empty() {
            h += 28.0;
        }
        constraints.clamp(Size::new(220.0, h.max(100.0)))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if !self.header_button_text.is_empty() && self.header_button_rect().contains(*pos) {
                    self.pending_action.set(Some(SelectableListAction::Header));
                    return EventResult::Handled;
                }

                if let Some(i) = self.row_index_at_y(pos.y) {
                    self.select(i, true);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let old_hover = self.hovered_index.get();
                let old_btn = self.hovered_header.get();

                let new_btn = if !self.header_button_text.is_empty() {
                    self.header_button_rect().contains(*pos)
                } else {
                    false
                };
                let new_hover = self.row_index_at_y(pos.y);

                self.hovered_index.set(new_hover);
                self.hovered_header.set(new_btn);
                if old_hover != new_hover || old_btn != new_btn {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::Wheel { delta, .. } => {
                let viewport_h = self.list_body_viewport_height();
                let dy = self.body_scroll.scroll_by_wheel(
                    delta.y,
                    self.items.len(),
                    self.item_stride(),
                    viewport_h,
                );
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::PointerLeave => {
                self.hovered_index.set(None);
                self.hovered_header.set(false);
                EventResult::Handled
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
                KeyCode::Up if !self.items.is_empty() => {
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Down if !self.items.is_empty() => {
                    self.move_active(true);
                    EventResult::Handled
                }
                KeyCode::Home if !self.items.is_empty() => {
                    self.select(0, false);
                    EventResult::Handled
                }
                KeyCode::End if !self.items.is_empty() => {
                    self.select(self.items.len() - 1, false);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.items.is_empty() => {
                    self.select(self.active_index, true);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space if !self.header_button_text.is_empty() => {
                    self.pending_action.set(Some(SelectableListAction::Header));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },

            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        match self.pending_action.take()? {
            SelectableListAction::Header => {
                Some(SemanticEvent::submit(id, self.header_button_text.clone()))
            }
            SelectableListAction::Row(index) => self
                .items
                .get(index)
                .map(|item| SemanticEvent::change(id, item.id.clone())),
        }
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let bg = ctx.tokens().color_bg_layout();
        let border = Color::from_rgb(40, 40, 45);
        let t_sec = ctx.tokens().color_text_secondary();
        let t_ter = ctx.tokens().color_text_tertiary();
        let t_pri = ctx.tokens().color_primary();

        ctx.fill_rect(frame, bg, None);
        ctx.fill_rect(Rect::new(frame.x + frame.w - 1.0, frame.y, 1.0, frame.h), border, None);

        let mut y = frame.y;

        if !self.header_button_text.is_empty() {
            let btn_frame = Rect::new(frame.x + 8.0, y + 8.0, frame.w - 16.0, 32.0);
            let btn_bg = if self.hovered_header.get() {
                Color::from_rgb(45, 45, 52)
            } else {
                Color::from_rgb(35, 35, 42)
            };
            ctx.fill_rect(btn_frame, btn_bg, Some(Radius::uniform(6.0)));
            ctx.draw_text("+", Point::new(btn_frame.x + 10.0, btn_frame.y + 7.0), t_sec, 15.0);
            ctx.draw_text(
                &self.header_button_text,
                Point::new(btn_frame.x + 28.0, btn_frame.y + 8.0),
                t_sec,
                13.0,
            );
            ctx.fill_rect(
                Rect::new(frame.x + 8.0, btn_frame.y + btn_frame.h + 8.0, frame.w - 16.0, 1.0),
                border,
                None,
            );
            y = btn_frame.y + btn_frame.h + 16.0;
        }

        let list_top = y;
        let footer_h = if self.footer_text.is_empty() { 0.0 } else { 28.0 };
        let list_viewport_h = frame.h - (list_top - frame.y) - footer_h;
        let list_clip = Rect::new(frame.x, list_top, frame.w, list_viewport_h);
        ctx.canvas_2d().push_clip(list_clip);

        let stride = self.item_stride();
        let scroll_offset = self.body_scroll.scroll_offset();
        let (start, end) = self
            .body_scroll
            .scroll_range(self.items.len(), stride, list_viewport_h);

        for i in start..end {
            let iy = list_top + i as f32 * stride - scroll_offset;
            if iy + self.item_height < list_top || iy > list_top + list_viewport_h {
                continue;
            }

            let is_active = i == self.active_index;
            let is_hover = self.hovered_index.get() == Some(i);
            let item_frame = Rect::new(frame.x + 8.0, iy, frame.w - 16.0, self.item_height);

            if is_active {
                ctx.fill_rect(item_frame, Color::from_rgba(55, 110, 255, 25), Some(Radius::uniform(6.0)));
                ctx.fill_rect(
                    Rect::new(item_frame.x, item_frame.y + 6.0, 3.0, self.item_height - 12.0),
                    t_pri,
                    Some(Radius::uniform(1.5)),
                );
            } else if is_hover {
                ctx.fill_rect(item_frame, Color::from_rgb(42, 42, 48), Some(Radius::uniform(6.0)));
            }

            let icon = self.items[i].icon.as_deref().unwrap_or("");
            let text_x = if icon.is_empty() {
                item_frame.x + 14.0
            } else {
                item_frame.x + 32.0
            };
            if !icon.is_empty() {
                ctx.draw_text(icon, Point::new(item_frame.x + 10.0, item_frame.y + 8.0), t_sec, 14.0);
            }

            let color = if is_active { t_pri } else { t_sec };
            ctx.draw_text(&self.items[i].text, Point::new(text_x, item_frame.y + 9.0), color, 13.0);
        }

        ctx.canvas_2d().pop_clip();

        if !self.footer_text.is_empty() {
            let fy = frame.y + frame.h - 24.0;
            ctx.draw_text(&self.footer_text, Point::new(frame.x + 12.0, fy), t_ter, 11.0);
        }
        if self.focused {
            ctx.stroke_rect(
                frame,
                t_pri,
                1.5,
                Some(Radius::uniform(ctx.tokens().border_radius_sm())),
            );
        }
    }
}
