//! Selectable list widget.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::{ComponentId, EventResult, SemanticEvent, SnapshotFields, SystemEvent, WidgetTree};

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
        if pos_y < list_top {
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
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
        pending_change: Cell<Option<usize>>,
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
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            last_frame: Cell::new(None),
            pending_change: Cell::new(None),
        }
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
            SystemEvent::PointerDown { pos, .. } => {
                if !self.header_button_text.is_empty() {
                    let btn_rect = Rect::new(8.0, 0.0, 204.0, 40.0);
                    if btn_rect.contains(*pos) {
                        return EventResult::Handled;
                    }
                }

                if let Some(i) = self.row_index_at_y(pos.y) {
                    self.active_index = i;
                    self.pending_change.set(Some(i));
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let old_hover = self.hovered_index.get();
                let old_btn = self.hovered_header.get();

                let new_btn = if !self.header_button_text.is_empty() {
                    Rect::new(8.0, 0.0, 204.0, 40.0).contains(*pos)
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

            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let index = self.pending_change.take()?;
        let value = self
            .items
            .get(index)
            .map(|item| item.id.clone())
            .unwrap_or_else(|| index.to_string());
        Some(SemanticEvent::change(id, value))
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
    }
}
