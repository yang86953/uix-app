//! Breadcrumb widget — 面包屑导航路径。

use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::core::widget::WidgetTree;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
};
use std::cell::Cell;

/// 面包屑的一项。
#[derive(Debug, Clone, PartialEq)]
pub struct BreadcrumbItem {
    pub title: String,
    pub active: bool,
}

impl BreadcrumbItem {
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            active: false,
        }
    }
    pub fn active(mut self) -> Self {
        self.active = true;
        self
    }
}

component! {
    /// Breadcrumb — 导航路径指示器。
    pub struct Breadcrumb {
        items: Vec<BreadcrumbItem>,
        separator: String,
        focused: bool,
        pending_change: Cell<Option<usize>>,
    }

    tab_index => (&self) -> i32 { i32::from(!self.items.is_empty()) }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    picture_policy => (&self) -> crate::draw::compositor::PicturePolicy {
        crate::draw::compositor::PicturePolicy::Eligible
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(index) = self.item_index_at(*pos) else {
                    return EventResult::NotHandled;
                };
                self.select(index, true);
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
            SystemEvent::KeyDown { key, .. } if !self.items.is_empty() => match key {
                KeyCode::Left => {
                    self.move_active(false);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.move_active(true);
                    EventResult::Handled
                }
                KeyCode::Home => {
                    self.select(0, false);
                    EventResult::Handled
                }
                KeyCode::End => {
                    self.select(self.items.len() - 1, false);
                    EventResult::Handled
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.pending_change.set(Some(self.active_index()));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        let index = self.pending_change.take()?;
        self.items
            .get(index)
            .map(|item| SemanticEvent::change(id, item.title.clone()))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let text_secondary = ctx.tokens().color_text_secondary();
        let text_color = ctx.tokens().color_text();
        let mut x = frame.x;
        let h = frame.h;
        for (i, item) in self.items.iter().enumerate() {
            let color = if item.active { text_color } else { text_secondary };
            let item_w = Self::text_width(&item.title, 7.5);
            ctx.text_center(&item.title, Rect::new(x, frame.y, item_w, h), color, 13.0);
            x += item_w;
            if i < self.items.len() - 1 {
                let sep_w = Self::text_width(&self.separator, 8.0);
                ctx.text_center(&self.separator, Rect::new(x, frame.y, sep_w, h), text_secondary, 12.0);
                x += sep_w;
            }
        }
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(
                frame,
                ctx.tokens().color_primary(),
                1.5,
                Some(crate::draw::Radius::uniform(
                    ctx.tokens().border_radius_sm(),
                )),
            );
        }
    }
}

impl Default for Breadcrumb {
    fn default() -> Self {
        Self::new()
    }
}

impl Breadcrumb {
    fn intrinsic_size(&self) -> Size {
        if self.items.is_empty() {
            return Size::zero();
        }
        let mut w = 0.0f32;
        for (i, item) in self.items.iter().enumerate() {
            w += Self::text_width(&item.title, 7.5);
            if i < self.items.len() - 1 {
                w += Self::text_width(&self.separator, 8.0);
            }
        }
        Size::new(w, 22.0)
    }

    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            separator: "/".to_string(),
            focused: false,
            pending_change: Cell::new(None),
        }
    }
    pub fn item(mut self, item: BreadcrumbItem) -> Self {
        self.items.push(item);
        self.normalize_active();
        self
    }
    pub fn items(mut self, items: Vec<BreadcrumbItem>) -> Self {
        self.items = items;
        self.normalize_active();
        self
    }
    pub fn separator(mut self, s: impl Into<String>) -> Self {
        self.separator = s.into();
        self
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let active_title = self.active_title().map(str::to_owned);
        self.items = next.items;
        self.normalize_active();
        if let Some(active_title) = active_title {
            if let Some(index) = self
                .items
                .iter()
                .position(|item| item.title == active_title)
            {
                self.set_active(index);
            }
        }
        self.separator = next.separator;
    }

    pub fn active_index(&self) -> usize {
        self.items.iter().position(|item| item.active).unwrap_or(0)
    }

    pub fn active_title(&self) -> Option<&str> {
        self.items
            .get(self.active_index())
            .map(|item| item.title.as_str())
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Breadcrumb {
            items: self.items.clone(),
            separator: self.separator.clone(),
        }
    }

    fn normalize_active(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let active = self.items.iter().rposition(|item| item.active).unwrap_or(0);
        self.set_active(active);
    }

    fn set_active(&mut self, index: usize) {
        for (item_index, item) in self.items.iter_mut().enumerate() {
            item.active = item_index == index;
        }
    }

    fn select(&mut self, index: usize, activate_unchanged: bool) {
        if self.items.is_empty() {
            return;
        }
        let index = index.min(self.items.len() - 1);
        let changed = index != self.active_index();
        self.set_active(index);
        if changed || activate_unchanged {
            self.pending_change.set(Some(index));
        }
    }

    fn move_active(&mut self, forward: bool) {
        let current = self.active_index();
        let next = if forward {
            (current + 1).min(self.items.len() - 1)
        } else {
            current.saturating_sub(1)
        };
        self.select(next, false);
    }

    fn item_index_at(&self, pos: Point) -> Option<usize> {
        if pos.y < 0.0 || pos.y >= self.intrinsic_size().h || pos.x < 0.0 {
            return None;
        }
        let mut x = 0.0;
        for (index, item) in self.items.iter().enumerate() {
            let item_width = Self::text_width(&item.title, 7.5);
            if pos.x >= x && pos.x < x + item_width {
                return Some(index);
            }
            x += item_width;
            if index + 1 < self.items.len() {
                x += Self::text_width(&self.separator, 8.0);
            }
        }
        None
    }

    fn text_width(text: &str, glyph_width: f32) -> f32 {
        text.chars().count() as f32 * glyph_width + 8.0
    }
}
