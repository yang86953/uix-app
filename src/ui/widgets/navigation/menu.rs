//! Menu widget - horizontal or vertical navigation menu.
//!
//! Supports hover highlight, active selection, disabled items, icons, and
//! keyboard navigation.
use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::SnapshotFields;
use crate::ui::{ComponentId, EventResult, KeyCode, SemanticEvent, SystemEvent, WidgetTree};
use std::cell::{Cell, RefCell};

/// Menu direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MenuMode {
    Horizontal,
    Vertical,
}

/// Single menu item.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub disabled: bool,
}

// Navigation menu component.
component! {
    pub struct Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        hovered_idx: Cell<usize>,
        focused: bool,
        item_h: f32,
        pending_change: RefCell<Option<String>>,
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown { pos, .. } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx {
                    if !self.items[i].disabled {
                        let old_key = self.active_key.clone();
                        self.active_key = self.items[i].key.clone();
                        self.hovered_idx.set(i);
                        if self.active_key != old_key {
                            self.pending_change.replace(Some(self.active_key.clone()));
                        }
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx {
                    self.hovered_idx.set(i);
                }
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.hovered_idx.set(usize::MAX);
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => { self.focused = true; EventResult::Handled }
            SystemEvent::FocusOut => { self.focused = false; EventResult::Handled }
            SystemEvent::KeyDown { key, .. } => {
                let cur_idx = self.item_index_of_key(&self.active_key).unwrap_or(0);
                match key {
                    KeyCode::Right => {
                        if self.mode == MenuMode::Horizontal {
                            let mut next = cur_idx + 1;
                            while next < self.items.len() && self.items[next].disabled {
                                next += 1;
                            }
                            if next < self.items.len() {
                                self.active_key = self.items[next].key.clone();
                                self.pending_change.replace(Some(self.active_key.clone()));
                            }
                        } else {
                            if let Some(i) = self.first_non_disabled() {
                                self.active_key = self.items[i].key.clone();
                                self.pending_change.replace(Some(self.active_key.clone()));
                            }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Left => {
                        if self.mode == MenuMode::Horizontal
                            && cur_idx > 0 {
                                let mut prev = cur_idx - 1;
                                loop {
                                    if !self.items[prev].disabled {
                                        self.active_key = self.items[prev].key.clone();
                                        self.pending_change.replace(Some(self.active_key.clone()));
                                        break;
                                    }
                                    if prev == 0 { break; }
                                    prev -= 1;
                                }
                            }
                        EventResult::Handled
                    }
                    KeyCode::Down => {
                        if self.mode == MenuMode::Vertical {
                            let mut next = cur_idx + 1;
                            while next < self.items.len() && self.items[next].disabled {
                                next += 1;
                            }
                            if next < self.items.len() {
                                self.active_key = self.items[next].key.clone();
                                self.pending_change.replace(Some(self.active_key.clone()));
                            }
                        } else {
                            if let Some(i) = self.first_non_disabled() {
                                self.active_key = self.items[i].key.clone();
                                self.pending_change.replace(Some(self.active_key.clone()));
                            }
                        }
                        EventResult::Handled
                    }
                    KeyCode::Up => {
                        if self.mode == MenuMode::Vertical
                            && cur_idx > 0 {
                                let mut prev = cur_idx - 1;
                                loop {
                                    if !self.items[prev].disabled {
                                        self.active_key = self.items[prev].key.clone();
                                        self.pending_change.replace(Some(self.active_key.clone()));
                                        break;
                                    }
                                    if prev == 0 { break; }
                                    prev -= 1;
                                }
                            }
                        EventResult::Handled
                    }
                    _ => EventResult::NotHandled,
                }
            }
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    wants_continuous_pointer_move => (&self) -> bool { true }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_tertiary();
        let active_key = &self.active_key;
        let hovered = self.hovered_idx.get();
        let r = Radius::uniform(ctx.tokens().border_radius_sm());

        match self.mode {
            MenuMode::Horizontal => {
                let mut cx = frame.x;
                for (i, item) in self.items.iter().enumerate() {
                    let iw = item.label.len() as f32 * 8.0 + 32.0;
                    let item_rect = Rect::new(cx, frame.y, iw, self.item_h);
                    let is_active = item.key == *active_key;
                    let is_hover = i == hovered && hovered < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    if is_active {
                        ctx.fill_rect(Rect::new(cx + 8.0, frame.y + self.item_h - 2.0, iw - 16.0, 2.0), primary, None);
                    }
                    // Center text horizontally with the shared text_center helper.
                    ctx.text_center(&item.label, item_rect, item_c, 14.0);
                    cx += iw;
                }
            }
            MenuMode::Vertical => {
                for (i, item) in self.items.iter().enumerate() {
                    let item_rect = Rect::new(frame.x, frame.y + i as f32 * self.item_h, frame.w, self.item_h);
                    let is_active = item.key == *active_key;
                    let is_hover = i == hovered && hovered < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    let item_y = frame.y + i as f32 * self.item_h;
                    let item_rect = Rect::new(frame.x, item_y, frame.w, self.item_h);
                    let text_y = ctx.visual_center_y(item_rect, 14.0);
                    if !item.icon.is_empty() {
                        let icon_str = crate::ui::widgets::icon::icon_char(&item.icon);
                        let saved = *ctx.font();
                        if let Some(fh) = crate::ui::widgets::icon::lucide_handle() {
                            ctx.set_font(fh);
                        }
                        ctx.draw_text(icon_str, Point::new(frame.x + 12.0, text_y), item_c, 14.0);
                        ctx.set_font(saved);
                    }
                    let label_x = frame.x + if item.icon.is_empty() { 16.0 } else { 36.0 };
                    ctx.draw_text(&item.label, Point::new(label_x, text_y), item_c, 14.0);
                }
            }
        }

        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, Some(r));
        }
    }
}

impl Menu {
    fn intrinsic_size(&self) -> Size {
        match self.mode {
            MenuMode::Horizontal => {
                let w = self
                    .items
                    .iter()
                    .map(|i| i.label.len() as f32 * 8.0 + 32.0)
                    .sum::<f32>();
                Size::new(w.max(100.0), self.item_h)
            }
            MenuMode::Vertical => Size::new(200.0, self.items.len() as f32 * self.item_h),
        }
    }

    fn item_at(&self, px: f32, py: f32) -> Option<usize> {
        match self.mode {
            MenuMode::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cx = 0.0f32;
                for (i, item) in self.items.iter().enumerate() {
                    let iw = item.label.len() as f32 * 8.0 + 32.0;
                    if px >= cx && px < cx + iw {
                        return Some(i);
                    }
                    cx += iw;
                }
                None
            }
            MenuMode::Vertical => {
                let idx = (py / self.item_h) as usize;
                if idx < self.items.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    fn item_index_of_key(&self, key: &str) -> Option<usize> {
        self.items.iter().position(|item| item.key == key)
    }

    fn first_non_disabled(&self) -> Option<usize> {
        self.items.iter().position(|item| !item.disabled)
    }
}

impl Default for Menu {
    fn default() -> Self {
        Self::new()
    }
}

impl Menu {
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            active_key: String::new(),
            mode: MenuMode::Horizontal,
            hovered_idx: Cell::new(usize::MAX),
            focused: false,
            item_h: 32.0,
            pending_change: RefCell::new(None),
        }
    }
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        self
    }
    pub fn add_item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        self
    }
    pub fn mode(mut self, m: MenuMode) -> Self {
        self.mode = m;
        self
    }
    pub fn active_key(mut self, key: &str) -> Self {
        self.active_key = key.to_string();
        self
    }
    pub fn get_active_key(&self) -> &str {
        &self.active_key
    }
    pub fn set_active_key(&mut self, key: &str) {
        self.active_key = key.to_string();
    }
    pub fn item_height(mut self, h: f32) -> Self {
        self.item_h = h;
        self
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Menu {
            items: self.items.clone(),
            mode: self.mode,
            item_h: self.item_h,
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/widgets/navigation/menu.rs"]
mod tests;
