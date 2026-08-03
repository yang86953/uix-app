use crate::component;
use crate::core::{Constraints, Rect, Size};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::{DropPosition, TreeNode, TreePointerAction};

pub(crate) const TREE_ROW_HEIGHT: f32 = 28.0;


pub(crate) struct FlatNode {
    pub(crate) title: String,
    pub(crate) key: String,
    pub(crate) icon: String,
    pub(crate) depth: usize,
    pub(crate) has_children: bool,
    pub(crate) expanded: bool,
    pub(crate) disabled: bool,
    pub(crate) checkable: bool,
    pub(crate) checked: bool,
}

component! {
    pub struct Tree {
        pub(crate) nodes: Vec<TreeNode>,
        pub(crate) flat: Vec<FlatNode>,
        pub(crate) selected_key: String,
        pub(crate) selected_keys: Vec<String>,
        pub(crate) expanded_keys: Vec<String>,
        pub(crate) multiple: bool,
        pub(crate) searchable: bool,
        pub(crate) search_query: String,
        pub(crate) draggable: bool,
        pub(crate) drop_callback: Option<Rc<dyn Fn(&str, &str, DropPosition)>>,
        pub(crate) expand_callback: Option<Rc<dyn Fn(&str)>>,
        pub(crate) focused: bool,
        pub(crate) hovered_action: Option<TreePointerAction>,
        pub(crate) pressed_action: Option<TreePointerAction>,
        pub(crate) dragged_key: Option<String>,
        pub(crate) pending_change: RefCell<Option<String>>,
        pub(crate) body_scroll: VirtualListScroll,
        pub(crate) scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) layout_requested: Cell<bool>,
        pub(crate) last_frame: Cell<Option<Rect>>,
    }

    tab_index => (&self) -> i32 { 1 }

    accepts_text_input => (&self) -> bool { self.searchable }

    text_input_cursor_rect => (&self) -> Rect {
        let frame = self.local_frame();
        let width = (self.search_query.chars().count() as f32 * 8.0 + 8.0)
            .clamp(8.0, (frame.w - 16.0).max(8.0));
        Rect::new(frame.x + 8.0 + width, frame.y + 6.0, 1.0, 20.0)
    }

    measure => (&self, constraints: Constraints) -> Size {
        constraints.clamp(self.intrinsic_size())
    }

    scroll_delta_for_dirty => (&self) -> Option<(f32, f32)> {
        let delta = self.scroll_delta_strip.get();
        if delta.0.abs() > 0.01 || delta.1.abs() > 0.01 {
            self.scroll_delta_strip.set((0.0, 0.0));
            Some(delta)
        } else {
            None
        }
    }

    viewport_scroll_offset => (&self) -> Option<(f32, f32)> {
        Some((0.0, self.body_scroll.scroll_offset()))
    }

    take_layout_request => (&mut self) -> bool {
        self.layout_requested.replace(false)
    }

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { pos, delta } => {
                let frame = self.local_frame();
                if !frame.contains(*pos)
                    || self.searchable
                        && pos.y < frame.y + self.search_height_for_intrinsic()
                {
                    return EventResult::NotHandled;
                }
                let viewport_h = self.body_viewport_height();
                let dy = self.body_scroll.scroll_by_wheel(
                    delta.y,
                    self.flat.len(),
                    TREE_ROW_HEIGHT,
                    viewport_h,
                );
                if dy.abs() > 0.01 {
                    self.push_scroll_delta(0.0, dy);
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                if let Some(action) = self.action_at_point(*pos) {
                    if self.draggable {
                        if let TreePointerAction::Select(key) = &action {
                            self.dragged_key = Some(key.clone());
                        }
                    }
                    self.hovered_action = Some(action.clone());
                    self.pressed_action = Some(action);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::PointerUp {
                pos,
                button: MouseButton::Left,
                ..
            } => {
                let Some(pressed) = self.pressed_action.take() else {
                    return EventResult::NotHandled;
                };
                let released = self.action_at_point(*pos);
                self.hovered_action.clone_from(&released);
                if let Some(source) = self.dragged_key.take() {
                    if let Some(TreePointerAction::Select(target)) = released.as_ref() {
                        if source != *target {
                            if let Some(callback) = self.drop_callback.as_ref() {
                                callback(&source, target, self.drop_position_at(*pos));
                            }
                            return EventResult::Handled;
                        }
                    }
                }
                if released.as_ref() == Some(&pressed) {
                    self.commit_pointer_action(pressed);
                }
                EventResult::Handled
            }
            SystemEvent::PointerMove { pos, .. } => {
                let hovered = self.action_at_point(*pos);
                if hovered != self.hovered_action {
                    self.hovered_action = hovered;
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::PointerLeave => {
                let changed = self.hovered_action.take().is_some()
                    | self.pressed_action.take().is_some()
                    | self.dragged_key.take().is_some();
                if changed {
                    EventResult::Handled
                } else {
                    EventResult::NotHandled
                }
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                self.hovered_action = None;
                self.pressed_action = None;
                self.dragged_key = None;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
                KeyCode::Backspace if self.searchable && !self.search_query.is_empty() => {
                    self.search_query.pop();
                    self.refresh_search();
                    EventResult::Handled
                }
                KeyCode::Escape if self.searchable && !self.search_query.is_empty() => {
                    self.search_query.clear();
                    self.refresh_search();
                    EventResult::Handled
                }
                KeyCode::Down => {
                    self.move_selection(true);
                    EventResult::Handled
                }
                KeyCode::Up => {
                    self.move_selection(false);
                    EventResult::Handled
                }
                KeyCode::Right => {
                    self.expand_or_descend();
                    EventResult::Handled
                }
                KeyCode::Left => {
                    self.collapse_or_ascend();
                    EventResult::Handled
                }
                KeyCode::Space | KeyCode::Enter => {
                    self.activate_current(*key == KeyCode::Space);
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            },
            SystemEvent::TextInput { text } | SystemEvent::Paste { text }
                if self.searchable && !text.is_empty() && !text.chars().any(char::is_control) =>
            {
                self.search_query.push_str(text);
                self.refresh_search();
                EventResult::Handled
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

    render => (&self, frame: Rect, ctx: &mut PaintContext, tree: &WidgetTree) {
        let frame = Self::normalized_frame(frame);
        if frame.w <= 0.0 || frame.h <= 0.0 {
            self.last_frame.set(Some(frame));
            return;
        }
        self.last_frame.set(Some(frame));
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_tertiary();
        let hover_fill = ctx.tokens().color_fill_tertiary();
        let pressed_fill = ctx.tokens().color_fill_secondary();

        let search_h = self.search_height_for_intrinsic();
        if self.searchable {
            let search_rect = Rect::new(frame.x, frame.y, frame.w, search_h);
            ctx.fill_rect(search_rect, ctx.tokens().color_bg_container(), None);
            ctx.stroke_rect(search_rect, ctx.tokens().color_border_secondary(), 1.0, None);
            let query = if self.search_query.is_empty() {
                "搜索"
            } else {
                &self.search_query
            };
            let query_color = if self.search_query.is_empty() { text_sec } else { text };
            Self::paint_single_line(
                ctx,
                query,
                Rect::new(frame.x + 8.0, frame.y, (frame.w - 16.0).max(0.0), search_h),
                query_color,
                13.0,
            );
        }

        let viewport_h = self.body_viewport_height();
        let (start, end) = self
            .body_scroll
            .scroll_range(self.flat.len(), TREE_ROW_HEIGHT, viewport_h);
        ctx.push_clip(frame);

        for i in start..end {
            let node = &self.flat[i];
            let y = frame.y + search_h + i as f32 * TREE_ROW_HEIGHT
                - self.body_scroll.scroll_offset();
            if y + TREE_ROW_HEIGHT < frame.y + search_h
                || y > frame.y + search_h + viewport_h
            {
                continue;
            }
            let is_selected = self.multiple && self.selected_keys.contains(&node.key)
                || (!self.multiple && node.key == self.selected_key);
            let geometry = self.row_geometry(frame, i, y);
            let action_matches_key = |action: &TreePointerAction| match action {
                TreePointerAction::Check(key)
                | TreePointerAction::Toggle(key)
                | TreePointerAction::Select(key) => key == &node.key,
            };

            if is_selected {
                ctx.fill_rect(geometry.row, fill, None);
            }
            if self
                .hovered_action
                .as_ref()
                .is_some_and(action_matches_key)
            {
                ctx.fill_rect(geometry.row, hover_fill, None);
            }
            if self
                .pressed_action
                .as_ref()
                .is_some_and(action_matches_key)
            {
                ctx.fill_rect(geometry.row, pressed_fill, None);
            }

            if self.focused
                && tree.keyboard_focus_visible()
                && self.multiple
                && node.key == self.selected_key
            {
                let inset = 0.5_f32.min(geometry.row.w * 0.5).min(geometry.row.h * 0.5);
                let focus = Rect::new(
                    geometry.row.x + inset,
                    geometry.row.y + inset,
                    (geometry.row.w - inset * 2.0).max(0.0),
                    (geometry.row.h - inset * 2.0).max(0.0),
                );
                if focus.w > 0.0 && focus.h > 0.0 {
                    ctx.stroke_rect(focus, primary, 1.0, None);
                }
            }

            if let Some(check) = geometry.check {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if node.checked {
                        "check-square"
                    } else {
                        "square"
                    },
                    check,
                    if node.checked { primary } else { text_sec },
                    13.0_f32.min(check.h * 0.65),
                );
            }

            if let Some(toggle) = geometry.toggle {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    if node.expanded { "chevron-down" } else { "chevron-right" },
                    toggle,
                    text_sec,
                    12.0_f32.min(toggle.h * 0.6),
                );
            }

            if let Some(icon) = geometry.icon {
                crate::ui::widgets::Icon::paint_in_frame(
                    ctx,
                    &node.icon,
                    icon,
                    text_sec,
                    12.0_f32.min(icon.h * 0.6),
                );
            }

            let tc = if node.disabled {
                text_sec
            } else if is_selected {
                primary
            } else {
                text
            };
            Self::paint_single_line(ctx, &node.title, geometry.title, tc, 13.0);
        }

        ctx.pop_clip();
        if self.focused && tree.keyboard_focus_visible() {
            let inset = 0.75_f32.min(frame.w * 0.5).min(frame.h * 0.5);
            let focus = Rect::new(
                frame.x + inset,
                frame.y + inset,
                (frame.w - inset * 2.0).max(0.0),
                (frame.h - inset * 2.0).max(0.0),
            );
            if focus.w > 0.0 && focus.h > 0.0 {
                ctx.stroke_rect(focus, primary, 1.5, None);
            }
        }
    }
}

