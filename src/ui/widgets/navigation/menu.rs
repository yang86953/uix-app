//! Menu widget - horizontal or vertical navigation menu.
//!
//! Supports hover highlight, active selection, disabled items, icons, and
//! keyboard navigation.
use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::draw::api::PaintContext;
use crate::draw::Radius;
use crate::ui::state::State;
use crate::ui::SnapshotFields;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;

/// Menu direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MenuMode {
    Horizontal,
    Vertical,
    /// Inline 菜单使用垂直布局并保持子项展开。
    Inline,
}

/// `Menu` key collection accepted by `selected_keys` and `open_keys`.
///
/// A plain collection keeps the existing one-shot builder form, while
/// `State<Vec<String>>` establishes a bidirectional controlled binding.
#[doc(hidden)]
pub trait MenuKeyCollection {
    fn menu_keys(&self) -> Vec<String>;

    fn menu_state(&self) -> Option<State<Vec<String>>> {
        None
    }
}

impl MenuKeyCollection for Vec<String> {
    fn menu_keys(&self) -> Vec<String> {
        self.clone()
    }
}

impl MenuKeyCollection for [String] {
    fn menu_keys(&self) -> Vec<String> {
        self.to_vec()
    }
}

impl<const N: usize> MenuKeyCollection for [String; N] {
    fn menu_keys(&self) -> Vec<String> {
        self.to_vec()
    }
}

impl MenuKeyCollection for State<Vec<String>> {
    fn menu_keys(&self) -> Vec<String> {
        self.get()
    }

    fn menu_state(&self) -> Option<State<Vec<String>>> {
        Some(self.clone())
    }
}

/// Single menu item.
#[derive(Debug, Clone, PartialEq)]
pub struct MenuItem {
    pub key: String,
    pub label: String,
    pub icon: String,
    pub children: Vec<MenuItem>,
    pub disabled: bool,
}

impl MenuItem {
    pub fn new(label: impl Into<String>) -> Self {
        let label = label.into();
        Self {
            key: label.clone(),
            label,
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
        }
    }

    pub fn children(mut self, children: Vec<Self>) -> Self {
        self.children = children;
        self
    }

    pub fn icon(mut self, icon: impl Into<String>) -> Self {
        self.icon = icon.into();
        self
    }
}

// Navigation menu component.
component! {
    pub struct Menu {
        items: Vec<MenuItem>,
        active_key: String,
        mode: MenuMode,
        selected_keys: Vec<String>,
        open_keys: Vec<String>,
        #[snapshot(skip)]
        selected_keys_binding: Option<State<Vec<String>>>,
        #[snapshot(skip)]
        open_keys_binding: Option<State<Vec<String>>>,
        #[snapshot(skip)]
        selected_keys_configured: bool,
        #[snapshot(skip)]
        open_keys_configured: bool,
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
        self.sync_bound_keys();
        match event {
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx {
                    let item = self.visible_items().get(i).map(|(item, _)| {
                        (item.disabled, item.key.clone(), !item.children.is_empty())
                    });
                    if let Some((disabled, key, has_children)) = item {
                        if disabled {
                            return EventResult::NotHandled;
                        }
                        self.select_key(key.clone());
                        if self.mode != MenuMode::Inline && has_children {
                            self.toggle_open_key(&key);
                        }
                        self.hovered_idx.set(i);
                        return EventResult::Handled;
                    }
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let idx = self.item_at(pos.x, pos.y);
                if let Some(i) = idx.filter(|&i| {
                    self.visible_items()
                        .get(i)
                        .is_some_and(|(item, _)| !item.disabled)
                }) {
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
                    | (MenuMode::Vertical | MenuMode::Inline, KeyCode::Down) => {
                        self.select_adjacent(true);
                        EventResult::Handled
                    }
                    (MenuMode::Horizontal, KeyCode::Left)
                    | (MenuMode::Vertical | MenuMode::Inline, KeyCode::Up) => {
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
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
                for (i, (item, depth)) in self.visible_items().iter().enumerate() {
                    let iw = Self::horizontal_item_width(item, *depth);
                    let item_rect = Rect::new(cx, frame.y, iw, self.item_h);
                    let is_active = item.key == *active_key || self.selected_keys.contains(&item.key);
                    let is_hover = i == hovered;
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    if is_active {
                        ctx.fill_rect(Rect::new(cx + 8.0, frame.y + self.item_h - 2.0, iw - 16.0, 2.0), primary, None);
                    }
                    if item.icon.is_empty() {
                        ctx.text_center(&item.label, item_rect, item_c, 14.0);
                    } else {
                        let text_width = item.label.len() as f32 * 8.0;
                        let content_width = 16.0 + 4.0 + text_width;
                        let content_x = cx + (iw - content_width) * 0.5;
                        crate::ui::widgets::icon::Icon::paint_in_frame(
                            ctx,
                            &item.icon,
                            Rect::new(content_x, frame.y, 16.0, self.item_h),
                            item_c,
                            14.0,
                        );
                        ctx.draw_text_in_frame(
                            &item.label,
                            Rect::new(content_x + 20.0, frame.y, text_width, self.item_h),
                            item_c,
                            14.0,
                        );
                    }
                    cx += iw;
                }
            }
            MenuMode::Vertical | MenuMode::Inline => {
                for (i, (item, depth)) in self.visible_items().iter().enumerate() {
                    let item_y = frame.y + i as f32 * self.item_h;
                    let item_rect = Rect::new(frame.x, item_y, frame.w, self.item_h);
                    let is_active = item.key == *active_key || self.selected_keys.contains(&item.key);
                    let is_hover = i == hovered;
                    let item_c = if item.disabled { text_sec } else if is_active { primary } else { text };
                    if is_active || is_hover {
                        ctx.fill_rect(item_rect, fill, Some(r));
                    }
                    let label_pad = if item.icon.is_empty() { 16.0 } else { 36.0 }
                        + *depth as f32 * 16.0;
                    if !item.icon.is_empty() {
                        let icon_rect = Rect::new(frame.x + 12.0, item_y, 16.0, self.item_h);
                        crate::ui::widgets::icon::Icon::paint_in_frame(
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

        if self.focused && tree.keyboard_focus_visible() {
            ctx.stroke_rect(frame, primary, 1.5, Some(r));
        }
    }
}

impl Menu {
    fn intrinsic_size(&self) -> Size {
        let items = self.visible_items();
        match self.mode {
            MenuMode::Horizontal => {
                let w = items
                    .iter()
                    .map(|(item, depth)| Self::horizontal_item_width(item, *depth))
                    .sum::<f32>();
                Size::new(w.max(100.0), self.item_h)
            }
            MenuMode::Vertical | MenuMode::Inline => {
                Size::new(200.0, items.len() as f32 * self.item_h)
            }
        }
    }

    fn item_at(&self, px: f32, py: f32) -> Option<usize> {
        let items = self.visible_items();
        match self.mode {
            MenuMode::Horizontal => {
                if py < 0.0 || py > self.item_h {
                    return None;
                }
                let mut cx = 0.0f32;
                for (i, (item, depth)) in items.iter().enumerate() {
                    let iw = Self::horizontal_item_width(item, *depth);
                    if px >= cx && px < cx + iw {
                        return Some(i);
                    }
                    cx += iw;
                }
                None
            }
            MenuMode::Vertical | MenuMode::Inline => {
                let idx = (py / self.item_h) as usize;
                if idx < items.len() && py >= 0.0 {
                    Some(idx)
                } else {
                    None
                }
            }
        }
    }

    fn item_index_of_key(&self, key: &str) -> Option<usize> {
        self.visible_items()
            .iter()
            .position(|(item, _)| item.key == key)
    }

    fn horizontal_item_width(item: &MenuItem, depth: usize) -> f32 {
        item.label.len() as f32 * 8.0
            + 32.0
            + depth as f32 * 16.0
            + if item.icon.is_empty() { 0.0 } else { 20.0 }
    }

    fn select_adjacent(&mut self, forward: bool) {
        let visible = self.visible_items();
        let enabled = visible
            .iter()
            .enumerate()
            .filter_map(|(index, (item, _))| (!item.disabled).then_some(index))
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
        let next_key = visible[enabled[next_position]].0.key.clone();
        self.select_key(next_key);
    }

    fn select_key(&mut self, key: String) {
        let changed = self.active_key != key || self.selected_keys != [key.clone()];
        self.active_key.clone_from(&key);
        self.selected_keys = vec![key.clone()];
        self.write_selected_keys();
        if changed {
            self.pending_change.replace(Some(key));
        }
    }

    fn toggle_open_key(&mut self, key: &str) {
        if let Some(index) = self.open_keys.iter().position(|open| open == key) {
            self.open_keys.remove(index);
        } else {
            self.open_keys.push(key.to_string());
        }
        self.write_open_keys();
    }

    fn visible_items(&self) -> Vec<(&MenuItem, usize)> {
        fn visit<'a>(
            items: &'a [MenuItem],
            open_keys: &[String],
            expand_all: bool,
            depth: usize,
            output: &mut Vec<(&'a MenuItem, usize)>,
        ) {
            for item in items {
                output.push((item, depth));
                if !item.children.is_empty() && (expand_all || open_keys.contains(&item.key)) {
                    visit(&item.children, open_keys, expand_all, depth + 1, output);
                }
            }
        }

        let mut output = Vec::new();
        visit(
            &self.items,
            &self.open_keys,
            self.mode == MenuMode::Inline,
            0,
            &mut output,
        );
        output
    }

    fn sync_bound_keys(&mut self) {
        if let Some(state) = self.selected_keys_binding.as_ref() {
            self.apply_selected_keys(state.get());
        }
        if let Some(state) = self.open_keys_binding.as_ref() {
            self.open_keys = Self::deduplicate_keys(state.get());
        }
    }

    fn apply_selected_keys(&mut self, keys: Vec<String>) {
        self.selected_keys = Self::deduplicate_keys(keys);
        self.active_key = self
            .selected_keys
            .iter()
            .find(|key| Self::contains_key(&self.items, key))
            .cloned()
            .unwrap_or_default();
    }

    fn deduplicate_keys(keys: Vec<String>) -> Vec<String> {
        let mut seen = HashSet::new();
        keys.into_iter()
            .filter(|key| seen.insert(key.clone()))
            .collect()
    }

    fn contains_key(items: &[MenuItem], key: &str) -> bool {
        items.iter().any(|item| {
            item.key == key
                || (!item.children.is_empty() && Self::contains_key(&item.children, key))
        })
    }

    fn write_selected_keys(&self) {
        if let Some(state) = self.selected_keys_binding.as_ref() {
            if state.get() != self.selected_keys {
                state.set(self.selected_keys.clone());
            }
        }
    }

    fn write_open_keys(&self) {
        if let Some(state) = self.open_keys_binding.as_ref() {
            if state.get() != self.open_keys {
                state.set(self.open_keys.clone());
            }
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
            selected_keys: Vec::new(),
            open_keys: Vec::new(),
            selected_keys_binding: None,
            open_keys_binding: None,
            selected_keys_configured: false,
            open_keys_configured: false,
            hovered_idx: Cell::new(usize::MAX),
            focused: false,
            item_h: 32.0,
            pending_change: RefCell::new(None),
        }
    }
    pub fn items(mut self, items: Vec<MenuItem>) -> Self {
        self.items = items;
        if self.selected_keys_configured {
            self.apply_selected_keys(self.selected_keys.clone());
        }
        self
    }
    pub fn add_item(mut self, item: MenuItem) -> Self {
        self.items.push(item);
        if self.selected_keys_configured {
            self.apply_selected_keys(self.selected_keys.clone());
        }
        self
    }
    pub fn mode(mut self, m: MenuMode) -> Self {
        self.mode = m;
        self
    }
    pub fn active_key(mut self, key: &str) -> Self {
        self.active_key = key.to_string();
        self.selected_keys_binding = None;
        self.selected_keys_configured = false;
        self
    }
    pub fn get_active_key(&self) -> &str {
        &self.active_key
    }
    pub fn set_active_key(&mut self, key: &str) {
        self.active_key = key.to_string();
        self.selected_keys = if key.is_empty() {
            Vec::new()
        } else {
            vec![key.to_string()]
        };
        self.write_selected_keys();
    }
    pub fn item_height(mut self, h: f32) -> Self {
        self.item_h = h;
        self
    }

    /// 设置选中 key；传入 `State<Vec<String>>` 时建立双向受控绑定。
    pub fn selected_keys<T>(mut self, keys: &T) -> Self
    where
        T: MenuKeyCollection + ?Sized,
    {
        self.selected_keys_binding = keys.menu_state();
        self.selected_keys_configured = true;
        self.apply_selected_keys(keys.menu_keys());
        self
    }

    /// 设置展开 key；传入 `State<Vec<String>>` 时建立双向受控绑定。
    pub fn open_keys<T>(mut self, keys: &T) -> Self
    where
        T: MenuKeyCollection + ?Sized,
    {
        self.open_keys_binding = keys.menu_state();
        self.open_keys_configured = true;
        self.open_keys = Self::deduplicate_keys(keys.menu_keys());
        self
    }

    pub fn get_selected_keys(&self) -> &[String] {
        &self.selected_keys
    }

    pub fn get_open_keys(&self) -> &[String] {
        &self.open_keys
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let previous_selected_keys = std::mem::take(&mut self.selected_keys);
        let previous_open_keys = std::mem::take(&mut self.open_keys);
        self.items = next.items;
        self.mode = next.mode;
        self.item_h = next.item_h;
        self.selected_keys_binding = next.selected_keys_binding;
        self.open_keys_binding = next.open_keys_binding;
        self.selected_keys_configured = next.selected_keys_configured;
        self.open_keys_configured = next.open_keys_configured;
        if self.selected_keys_configured {
            self.apply_selected_keys(next.selected_keys);
        } else {
            self.selected_keys = previous_selected_keys;
        }
        self.open_keys = if self.open_keys_configured {
            next.open_keys
        } else {
            previous_open_keys
        };
        self.hovered_idx.set(usize::MAX);
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
