//! Selectable list widget.

use std::cell::Cell;

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::{EventResult, SemanticEvent, SnapshotFields, SystemEvent, WidgetId, WidgetTree};

const DEFAULT_SCROLL_VIEWPORT_HEIGHT: f32 = 500.0;

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

    fn max_scroll_offset(&self) -> f32 {
        let content_height = self.items.len() as f32 * (self.item_height + 2.0);
        (content_height - DEFAULT_SCROLL_VIEWPORT_HEIGHT).max(0.0)
    }

    fn push_scroll_delta(&self, dy: f32) {
        if dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip.set((current.0, current.1 + dy));
    }

    fn scroll_by(&self, dy: f32) -> bool {
        let old_offset = -self.scroll_y.get();
        let new_offset = (old_offset + dy).clamp(0.0, self.max_scroll_offset());
        let actual_dy = new_offset - old_offset;
        if actual_dy.abs() <= 0.01 {
            return false;
        }
        self.scroll_y.set(-new_offset);
        self.push_scroll_delta(actual_dy);
        true
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
        scroll_y: Cell<f32>,
        scroll_delta_strip: Cell<(f32, f32)>,
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
            scroll_y: Cell::new(0.0),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            pending_change: Cell::new(None),
        }
    }

    measure => (&self, constraints: Constraints) -> Size {
        let mut h = 0.0;
        if !self.header_button_text.is_empty() {
            h += 48.0;
        }
        h += self.items.len() as f32 * (self.item_height + 2.0);
        if !self.footer_text.is_empty() {
            h += 28.0;
        }
        constraints.clamp(Size::new(220.0, h.max(100.0)))
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let mut y = 0.0;

                if !self.header_button_text.is_empty() {
                    let btn_rect = Rect::new(8.0, y, 204.0, 40.0);
                    if btn_rect.contains(*pos) {
                        return EventResult::Handled;
                    }
                    y += 48.0;
                }

                for i in 0..self.items.len() {
                    let iy = y + i as f32 * (self.item_height + 2.0) + self.scroll_y.get();
                    let item_rect = Rect::new(0.0, iy, 220.0, self.item_height);
                    if item_rect.contains(*pos) {
                        self.active_index = i;
                        self.pending_change.set(Some(i));
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }

            SystemEvent::PointerMove { pos, .. } => {
                let old_hover = self.hovered_index.get();
                let old_btn = self.hovered_header.get();
                let mut new_hover = None;
                let mut new_btn = false;

                let mut y = 0.0;
                if !self.header_button_text.is_empty() {
                    let btn_rect = Rect::new(8.0, y, 204.0, 40.0);
                    if btn_rect.contains(*pos) {
                        new_btn = true;
                    }
                    y += 48.0;
                }

                for i in 0..self.items.len() {
                    let iy = y + i as f32 * (self.item_height + 2.0) + self.scroll_y.get();
                    let item_rect = Rect::new(0.0, iy, 220.0, self.item_height);
                    if item_rect.contains(*pos) {
                        new_hover = Some(i);
                        break;
                    }
                }

                self.hovered_index.set(new_hover);
                self.hovered_header.set(new_btn);
                if old_hover != new_hover || old_btn != new_btn {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }

            SystemEvent::Wheel { delta, .. } => {
                if self.scroll_by(delta.y * 0.5) {
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

    semantic_event => (&self, id: WidgetId, _event: &SystemEvent) -> Option<SemanticEvent> {
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

        let sy = self.scroll_y.get();
        let list_clip = Rect::new(frame.x, y, frame.w, frame.h - y - 28.0);
        ctx.canvas_2d().push_clip(list_clip);

        for i in 0..self.items.len() {
            let iy = y + i as f32 * (self.item_height + 2.0) + sy;
            if iy + self.item_height < y || iy > y + list_clip.h {
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

#[cfg(test)]
#[path = "../../../tests/ui/widgets/display/selectable_list.rs"]
mod tests;
