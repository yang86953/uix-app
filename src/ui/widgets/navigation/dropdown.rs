//! Dropdown widget.

use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::{Color, Radius};
use crate::ui::animation::{presets, TransitionPlayer};
use crate::ui::core::paint_context::PaintContext;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields, SystemEvent,
    WidgetTree,
};
use std::cell::RefCell;

use crate::ui::widgets::feedback::TriggerMode;

#[derive(Debug, Clone, PartialEq)]
pub struct DropdownItem {
    pub label: String,
    pub divider: bool,
    pub disabled: bool,
    pub icon: String,
    pub children: Vec<DropdownItem>,
}

impl DropdownItem {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            divider: false,
            disabled: false,
            icon: String::new(),
            children: Vec::new(),
        }
    }

    pub fn divider() -> Self {
        Self {
            label: String::new(),
            divider: true,
            disabled: false,
            icon: String::new(),
            children: Vec::new(),
        }
    }

    pub fn children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

impl From<DropdownItem> for String {
    fn from(item: DropdownItem) -> Self {
        item.label
    }
}

impl From<&str> for DropdownItem {
    fn from(label: &str) -> Self {
        Self::new(label)
    }
}

impl From<String> for DropdownItem {
    fn from(label: String) -> Self {
        Self::new(label)
    }
}

#[derive(Clone)]
struct VisibleDropdownItem {
    item: DropdownItem,
    depth: usize,
}

component! {
    /// Click-triggered dropdown menu.
    pub struct Dropdown {
        label: String,
        items: Vec<DropdownItem>,
        expanded_keys: Vec<String>,
        open: bool,
        transition: TransitionPlayer,
        closing: bool,
        transition_dirty: bool,
        focused: bool,
        selected_index: Option<usize>,
        selected_value: Option<String>,
        highlighted_index: Option<usize>,
        trigger_mode: TriggerMode,
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
                button,
                ..
            } => {
                if pos.y >= 0.0 && pos.y <= 32.0 {
                    let accepted_trigger = match self.trigger_mode {
                        TriggerMode::ContextMenu => *button == MouseButton::Right,
                        _ => *button == MouseButton::Left,
                    };
                    if !accepted_trigger {
                        return EventResult::NotHandled;
                    }
                    if self.open {
                        self.close();
                    } else {
                        self.open();
                    }
                    return EventResult::Handled;
                }
                if self.open && pos.y > 32.0 && *button == MouseButton::Left {
                    if let Some(index) = self.item_at_y(pos.y) {
                        if self.toggle_or_select(index) {
                            return EventResult::Handled;
                        }
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp { .. } if self.trigger_mode == TriggerMode::ContextMenu => {
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                if self.trigger_mode == TriggerMode::Hover && !self.open {
                    self.open();
                }
                self.highlighted_index = self.item_at_y(pos.y);
                EventResult::Handled
            }
            SystemEvent::PointerEnter if self.trigger_mode == TriggerMode::Hover => {
                self.open();
                EventResult::Handled
            }
            SystemEvent::PointerLeave => {
                self.highlighted_index = self.selected_index;
                if self.trigger_mode == TriggerMode::Hover {
                    self.close();
                }
                EventResult::Handled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                if self.trigger_mode == TriggerMode::Focus {
                    self.open();
                }
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
                            self.toggle_or_select(index);
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
                KeyCode::Home if self.open => {
                    self.highlighted_index = self.next_selectable(None, true);
                    EventResult::Handled
                }
                KeyCode::End if self.open => {
                    self.highlighted_index = self.next_selectable(None, false);
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
        let visible = self.visible_items();
        let opacity = self.transition.opacity_progress.clamp(0.0, 1.0);
        let bg = fade_color(ctx.tokens().color_bg_elevated(), opacity);
        let border = fade_color(ctx.tokens().color_border(), opacity);
        let text_color = fade_color(ctx.tokens().color_text(), opacity);
        let disabled_color = fade_color(ctx.tokens().color_text_quaternary(), opacity);
        let highlight = fade_color(ctx.tokens().color_fill_tertiary(), opacity);

        let menu_y = frame.y + 32.0;
        let menu_h = visible.iter().map(Self::row_height).sum::<f32>();
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

        let mut y = menu_y;
        for (index, row) in visible.iter().enumerate() {
            let h = Self::row_height(row);
            let item_rect = Rect::new(frame.x, y, frame.w, h);
            if row.item.divider {
                let inset = 8.0_f32.min(frame.w * 0.5);
                ctx.fill_rect(
                    Rect::new(frame.x + inset, y + h * 0.5, (frame.w - inset * 2.0).max(0.0), 1.0),
                    border,
                    None,
                );
            } else {
                if self.highlighted_index == Some(index) || self.selected_index == Some(index) {
                    ctx.fill_rect(item_rect, highlight, r);
                }
                let color = if row.item.disabled { disabled_color } else { text_color };
                let mut content_x = frame.x + 12.0 + row.depth as f32 * 16.0;
                if !row.item.icon.is_empty() {
                    let icon_rect = Rect::new(content_x, y, 20.0, h);
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        &row.item.icon,
                        icon_rect,
                        color,
                        14.0,
                    );
                    content_x += 24.0;
                }
                let arrow_space = if row.item.children.is_empty() { 0.0 } else { 20.0 };
                let label_rect = Rect::new(
                    content_x,
                    y,
                    (frame.x + frame.w - content_x - arrow_space - 8.0).max(0.0),
                    h,
                );
                ctx.draw_text_in_frame(&row.item.label, label_rect, color, 13.0);
                if !row.item.children.is_empty() {
                    crate::ui::widgets::icon::Icon::paint_in_frame(
                        ctx,
                        if self.expanded_keys.iter().any(|key| key == &row.item.label) {
                            "chevron-down"
                        } else {
                            "chevron-right"
                        },
                        Rect::new(frame.x + frame.w - 24.0, y, 16.0, h),
                        color,
                        12.0,
                    );
                }
            }
            y += h;
        }
    }

    hit_test_frame => (&self, frame: Rect) -> Rect {
        if self.is_present() {
            let menu_h = self.visible_items().iter().map(Self::row_height).sum::<f32>();
            Rect::new(frame.x, frame.y, frame.w, 32.0 + menu_h)
        } else {
            frame
        }
    }

    dirty_rect => (&self, frame: Rect) -> Rect {
        dropdown_dirty_rect(
            frame,
            self.visible_items().iter().map(Self::row_height).sum(),
        )
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
            dropdown_dirty_rect(
                frame,
                self.visible_items().iter().map(Self::row_height).sum(),
            )
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
            expanded_keys: Vec::new(),
            open: false,
            transition: TransitionPlayer::new(presets::tooltip_enter()),
            closing: false,
            transition_dirty: false,
            focused: false,
            selected_index: None,
            selected_value: None,
            highlighted_index: None,
            trigger_mode: TriggerMode::Click,
            pending_change: RefCell::new(None),
        }
    }

    pub fn items<T>(mut self, items: Vec<T>) -> Self
    where
        T: Into<DropdownItem>,
    {
        self.items = items.into_iter().map(|s| s.into()).collect();
        self
    }

    pub fn trigger(mut self, trigger: TriggerMode) -> Self {
        self.trigger_mode = trigger;
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
        self.selected_value.as_deref()
    }

    pub fn open(&mut self) {
        self.open = true;
        self.closing = false;
        self.highlighted_index = self
            .selected_index
            .filter(|index| self.is_selectable(*index))
            .or_else(|| self.next_selectable(None, true));
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
        self.expanded_keys = next.expanded_keys;
        self.trigger_mode = next.trigger_mode;
        self.selected_index = selected_value.as_ref().and_then(|value| {
            self.visible_items()
                .iter()
                .position(|row| row.item.label == *value)
        });
        self.selected_value = selected_value;
        self.highlighted_index = if self.open {
            self.selected_index
                .filter(|index| self.is_selectable(*index))
                .or_else(|| self.next_selectable(None, true))
        } else {
            None
        };
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Dropdown {
            label: self.label.clone(),
            items: self.items.iter().map(|item| item.label.clone()).collect(),
            open: self.open,
            selected_index: self.selected_index,
            highlighted_index: self.highlighted_index,
        }
    }

    fn item_at_y(&self, y: f32) -> Option<usize> {
        if !self.open || y <= 32.0 {
            return None;
        }
        let mut cursor = 32.0;
        for (index, row) in self.visible_items().iter().enumerate() {
            let height = Self::row_height(row);
            if y >= cursor && y < cursor + height {
                return Some(index);
            }
            cursor += height;
        }
        None
    }

    fn move_highlight(&mut self, forward: bool) {
        if !self.open {
            self.open();
            return;
        }
        self.highlighted_index = self.next_selectable(self.highlighted_index, forward);
    }

    fn select_index(&mut self, index: usize) -> bool {
        let rows = self.visible_items();
        let Some(row) = rows.get(index) else {
            return false;
        };
        if row.item.divider || row.item.disabled || !row.item.children.is_empty() {
            return false;
        }
        let value = row.item.label.clone();
        self.selected_index = Some(index);
        self.selected_value = Some(value.clone());
        self.highlighted_index = Some(index);
        self.pending_change.replace(Some(value));
        self.close();
        true
    }

    fn toggle_or_select(&mut self, index: usize) -> bool {
        let Some(row) = self.visible_items().get(index).cloned() else {
            return false;
        };
        if row.item.divider || row.item.disabled {
            return false;
        }
        if !row.item.children.is_empty() {
            if self.expanded_keys.iter().any(|key| key == &row.item.label) {
                self.expanded_keys.retain(|key| key != &row.item.label);
            } else {
                self.expanded_keys.push(row.item.label);
            }
            self.highlighted_index = Some(index);
            return true;
        }
        self.select_index(index)
    }

    fn is_selectable(&self, index: usize) -> bool {
        self.visible_items()
            .get(index)
            .is_some_and(|row| !row.item.divider && !row.item.disabled)
    }

    fn next_selectable(&self, current: Option<usize>, forward: bool) -> Option<usize> {
        let rows = self.visible_items();
        if rows.is_empty() {
            return None;
        }
        let start = match (current, forward) {
            (Some(index), true) => (index + 1) % rows.len(),
            (Some(index), false) => (index + rows.len() - 1) % rows.len(),
            (None, true) => 0,
            (None, false) => rows.len() - 1,
        };
        for offset in 0..rows.len() {
            let index = if forward {
                (start + offset) % rows.len()
            } else {
                (start + rows.len() - offset) % rows.len()
            };
            if !rows[index].item.divider && !rows[index].item.disabled {
                return Some(index);
            }
        }
        None
    }

    fn visible_items(&self) -> Vec<VisibleDropdownItem> {
        fn visit(
            items: &[DropdownItem],
            expanded_keys: &[String],
            depth: usize,
            rows: &mut Vec<VisibleDropdownItem>,
        ) {
            for item in items {
                let expanded = expanded_keys.iter().any(|key| key == &item.label);
                let children = item.children.clone();
                rows.push(VisibleDropdownItem {
                    item: item.clone(),
                    depth,
                });
                if expanded && !children.is_empty() {
                    visit(&children, expanded_keys, depth + 1, rows);
                }
            }
        }

        let mut rows = Vec::new();
        visit(&self.items, &self.expanded_keys, 0, &mut rows);
        rows
    }

    fn row_height(row: &VisibleDropdownItem) -> f32 {
        if row.item.divider {
            8.0
        } else {
            30.0
        }
    }
}

fn dropdown_dirty_rect(frame: Rect, menu_h: f32) -> Rect {
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
