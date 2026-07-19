//! Dropdown widget.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::RefCell;

component! {
    /// Click-triggered dropdown menu.
    pub struct Dropdown {
        label: String,
        items: Vec<String>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        selected_index: Option<usize>,
        highlighted_index: Option<usize>,
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
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                if self.open && pos.y > 32.0 {
                    let idx = ((pos.y - 32.0) / 30.0) as usize;
                    if self.select_index(idx) {
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                self.highlighted_index = self.item_at_y(pos.y);
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.highlighted_index = self.selected_index;
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.close();
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Enter | KeyCode::Space => {
                    if self.open {
                        if let Some(index) = self.highlighted_index {
                            self.select_index(index);
                        }
                    } else {
                        self.open();
                    }
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.move_highlight(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_highlight(false);
                    EventResult::Handled
                }
                KeyCode::Home if self.open && !self.items.is_empty() => {
                    self.highlighted_index = Some(0);
                    EventResult::Handled
                }
                KeyCode::End if self.open && !self.items.is_empty() => {
                    self.highlighted_index = Some(self.items.len() - 1);
                    EventResult::Handled
                }
                KeyCode::Escape if self.is_present() => {
                    self.close();
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|value| SemanticEvent::change(id, value))
    }

    wants_continuous_pointer_move => (&self) -> bool { self.open }

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let r = Some(Radius::uniform(ctx.tokens().border_radius_sm()));

        let btn_rect = Rect::new(frame.x, frame.y, frame.w, 32.0);
        ctx.fill_rect(btn_rect, ctx.tokens().color_primary(), r);
        ctx.text_center(&self.label, btn_rect, crate::draw::Color::white(), 13.0);
        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(btn_rect, ctx.tokens().color_primary_active(), 1.5, r);
        }

        if !self.is_present() {
            return;
        }
        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
        let border = fade_color(ctx.tokens().color_border(), opacity);
        let text_color = fade_color(ctx.tokens().color_text(), opacity);
        let highlight = fade_color(ctx.tokens().color_fill_tertiary(), opacity);

        let menu_y = frame.y + 32.0;
        let menu_h = self.items.len() as f32 * 30.0;
        let menu_rect = Rect::new(frame.x, menu_y, frame.w, menu_h);
        let shadow = ctx.tokens().box_shadow_secondary();
        ctx.draw_box_shadow(
            menu_rect,
            shadow.layer_1.2,
            shadow.layer_1.0,
            shadow.layer_1.1,
            shadow.layer_1.3,
            r,
        );
        ctx.fill_rect(menu_rect, bg, r);
        ctx.stroke_rect(menu_rect, border, 1.0, r);

        for (i, item) in self.items.iter().enumerate() {
            let item_rect = Rect::new(frame.x, menu_y + i as f32 * 30.0, frame.w, 30.0);
            if self.highlighted_index == Some(i) || self.selected_index == Some(i) {
                ctx.fill_rect(item_rect, highlight, r);
            }
            ctx.text_center(item, item_rect, text_color, 13.0);
        }
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            let menu_h = self.items.len() as f32 * 30.0;
            Rect::new(frame.x, frame.y, frame.w, 32.0 + menu_h)
        } else {
            frame
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        dropdown_dirty_rect(frame, self.items.len())
    }

    update_animation => (&mut self, dt: f64) -> bool {
        if !self.is_present() || self.transition.finished {
            self.transition_dirty = false;
            return false;
        }

        self.transition.update(dt);
        self.transition_dirty = true;

        if self.closing && self.transition.finished {
            self.open = false;
            self.closing = false;
        }

        self.is_present() && !self.transition.finished
    }

    dirty_bounds => (&self, frame: Rect) -> Rect {
        if self.transition_dirty {
            dropdown_dirty_rect(frame, self.items.len())
        } else {
            Rect::zero()
        }
    }
}

impl Default for Dropdown {
    fn default() -> Self {
        Self::new("Menu")
    }
}

impl Dropdown {
    fn intrinsic_size(&self) -> Size {
        Size::new(160.0, 32.0)
    }

    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            items: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            selected_index: None,
            highlighted_index: None,
            pending_change: RefCell::new(None),
        }
    }

    pub fn items(mut self, items: Vec<impl Into<String>>) -> Self {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn is_present(&self) -> bool {
        self.open || self.closing
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected_index
    }

    pub fn current_value(&self) -> Option<&str> {
        self.selected_index
            .and_then(|index| self.items.get(index))
            .map(String::as_str)
    }

    pub fn open(&mut self) {
        self.open = true;
        self.closing = false;
        self.highlighted_index = self
            .selected_index
            .or_else(|| (!self.items.is_empty()).then_some(0));
        self.transition = TransitionPlayer::new(presets::tooltip_enter());
        self.transition_dirty = true;
    }

    pub fn close(&mut self) {
        if !self.is_present() {
            self.open = false;
            self.closing = false;
            self.transition_dirty = false;
            return;
        }
        self.open = false;
        self.closing = true;
        self.transition = TransitionPlayer::new(presets::tooltip_exit());
        self.transition_dirty = true;
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let selected_value = self.current_value().map(str::to_owned);
        self.label = next.label;
        self.items = next.items;
        self.selected_index = selected_value
            .as_ref()
            .and_then(|value| self.items.iter().position(|item| item == value));
        self.highlighted_index = if self.open {
            self.selected_index
                .or_else(|| (!self.items.is_empty()).then_some(0))
        } else {
            None
        };
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Dropdown {
            label: self.label.clone(),
            items: self.items.clone(),
            open: self.open,
            selected_index: self.selected_index,
            highlighted_index: self.highlighted_index,
        }
    }

    fn item_at_y(&self, y: f32) -> Option<usize> {
        if !self.open || y <= 32.0 {
            return None;
        }
        let index = ((y - 32.0) / 30.0) as usize;
        (index < self.items.len()).then_some(index)
    }

    fn move_highlight(&mut self, forward: bool) {
        if self.items.is_empty() {
            return;
        }
        if !self.open {
            self.open();
            return;
        }
        let index = match (self.highlighted_index, forward) {
            (Some(index), true) => (index + 1) % self.items.len(),
            (Some(index), false) => (index + self.items.len() - 1) % self.items.len(),
            (None, true) => 0,
            (None, false) => self.items.len() - 1,
        };
        self.highlighted_index = Some(index);
    }

    fn select_index(&mut self, index: usize) -> bool {
        let Some(value) = self.items.get(index).cloned() else {
            return false;
        };
        self.selected_index = Some(index);
        self.highlighted_index = Some(index);
        self.pending_change.replace(Some(value));
        self.close();
        true
    }
}

fn dropdown_dirty_rect(frame: Rect, item_count: usize) -> Rect {
    let menu_h = item_count as f32 * 30.0;
    let menu = Rect::new(frame.x, frame.y + 32.0, frame.w, menu_h);
    let expanded = frame.union(&menu);
    let expand = 8.0;
    Rect::new(
        expanded.x - expand,
        expanded.y - expand,
        expanded.w + expand * 2.0,
        expanded.h + expand * 2.0,
    )
}

fn fade_color(color: Color, opacity: f32) -> Color {
    let alpha = (color.a as f32 * opacity.clamp(0.0, 1.0))
        .round()
        .clamp(0.0, 255.0) as u8;
    color.with_alpha(alpha)
}
