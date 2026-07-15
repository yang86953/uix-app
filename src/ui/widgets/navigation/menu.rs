//! Menu widget - horizontal or vertical navigation menu.
//!
//! Supports hover highlight, active selection, disabled items, icons, and
//! keyboard navigation.
use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::Radius;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
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
                if let Some(i) = idx.filter(|&i| !self.items[i].disabled) {
                    self.hovered_idx.set(i);
                } else {
                    self.hovered_idx.set(usize::MAX);
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
                match (self.mode, key) {
                    (MenuMode::Horizontal, KeyCode::Right)
                    | (MenuMode::Vertical, KeyCode::Down) => {
                        self.select_adjacent(true);
                        EventResult::Handled
                    }
                    (MenuMode::Horizontal, KeyCode::Left)
                    | (MenuMode::Vertical, KeyCode::Up) => {
                        self.select_adjacent(false);
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
                    let item_y = frame.y + i as f32 * self.item_h;
                    let item_rect = Rect::new(frame.x, item_y, frame.w, self.item_h);
                    let is_active = item.key == *active_key;
                    let is_hover = i == hovered && hovered < self.items.len();
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    let label_pad = if item.icon.is_empty() { 16.0 } else { 36.0 };
                    if !item.icon.is_empty() {
                        let icon_rect = Rect::new(frame.x + 12.0, item_y, 16.0, self.item_h);
                        crate::ui::widgets::icon::paint_icon_in_frame(
                            ctx, &item.icon, icon_rect, item_c, 14.0,
                        );
                    }
                    let label_rect = Rect::new(
                        frame.x + label_pad,
                        item_y,
                        (frame.w - label_pad - 8.0).max(0.0),
                        self.item_h,
                    );
                    ctx.draw_text_in_frame(&item.label, label_rect, item_c, 14.0);
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

    fn select_adjacent(&mut self, forward: bool) {
        let enabled = self
            .items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!item.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return;
        }

        let current = self.item_index_of_key(&self.active_key);
        let next_position = current
            .and_then(|index| enabled.iter().position(|&candidate| candidate == index))
            .map(|position| {
                if forward {
                    (position + 1) % enabled.len()
                } else {
                    (position + enabled.len() - 1) % enabled.len()
                }
            })
            .unwrap_or_else(|| if forward { 0 } else { enabled.len() - 1 });
        let next_key = self.items[enabled[next_position]].key.clone();
        if next_key != self.active_key {
            self.active_key = next_key.clone();
            self.pending_change.replace(Some(next_key));
        }
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

    pub(crate) fn sync_from(&mut self, next: Self) {
        self.items = next.items;
        self.mode = next.mode;
        self.item_h = next.item_h;
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Menu {
            items: self.items.clone(),
            active_key: self.active_key.clone(),
            mode: self.mode,
            item_h: self.item_h,
        }
    }
}
