use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::ui::component::paint_context::PaintContext;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTreeNode, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

pub(crate) const TREE_ROW_HEIGHT: f32 = 28.0;
const TREE_SLOT_WIDTH: f32 = 20.0;
const TREE_INDENT_WIDTH: f32 = 20.0;
const TREE_MIN_TITLE_WIDTH: f32 = 24.0;

#[derive(Debug, Clone, PartialEq, Eq)]
enum TreePointerAction {
    Check(String),
    Toggle(String),
    Select(String),
}

struct TreeRowGeometry {
    row: Rect,
    check: Option<Rect>,
    toggle: Option<Rect>,
    icon: Option<Rect>,
    title: Rect,
}

pub struct TreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<TreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub draggable: bool,
    pub lazy: bool,
    pub is_leaf: bool,
    filter: Option<TreeFilter>,
}

type TreeFilter = Rc<dyn Fn(&TreeNode, &str) -> bool>;

impl Clone for TreeNode {
    fn clone(&self) -> Self {
        Self {
            title: self.title.clone(),
            key: self.key.clone(),
            icon: self.icon.clone(),
            children: self.children.clone(),
            disabled: self.disabled,
            checkable: self.checkable,
            checked: self.checked,
            draggable: self.draggable,
            lazy: self.lazy,
            is_leaf: self.is_leaf,
            filter: self.filter.clone(),
        }
    }
}

impl std::fmt::Debug for TreeNode {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TreeNode")
            .field("title", &self.title)
            .field("key", &self.key)
            .field("icon", &self.icon)
            .field("children", &self.children)
            .field("disabled", &self.disabled)
            .field("checkable", &self.checkable)
            .field("checked", &self.checked)
            .field("draggable", &self.draggable)
            .field("lazy", &self.lazy)
            .field("is_leaf", &self.is_leaf)
            .field("has_filter", &self.filter.is_some())
            .finish()
    }
}

impl PartialEq for TreeNode {
    fn eq(&self, other: &Self) -> bool {
        self.title == other.title
            && self.key == other.key
            && self.icon == other.icon
            && self.children == other.children
            && self.disabled == other.disabled
            && self.checkable == other.checkable
            && self.checked == other.checked
            && self.draggable == other.draggable
            && self.lazy == other.lazy
            && self.is_leaf == other.is_leaf
            && self.filter.is_some() == other.filter.is_some()
    }
}

/// 树节点拖拽放置位置。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DropPosition {
    Before,
    Inside,
    After,
}

pub(crate) struct FlatNode {
    title: String,
    key: String,
    icon: String,
    depth: usize,
    has_children: bool,
    expanded: bool,
    disabled: bool,
    checkable: bool,
    checked: bool,
}

component! {
    pub struct Tree {
        nodes: Vec<TreeNode>,
        pub(crate) flat: Vec<FlatNode>,
        selected_key: String,
        selected_keys: Vec<String>,
        expanded_keys: Vec<String>,
        multiple: bool,
        searchable: bool,
        search_query: String,
        draggable: bool,
        drop_callback: Option<Rc<dyn Fn(&str, &str, DropPosition)>>,
        expand_callback: Option<Rc<dyn Fn(&str)>>,
        focused: bool,
        hovered_action: Option<TreePointerAction>,
        pressed_action: Option<TreePointerAction>,
        dragged_key: Option<String>,
        pending_change: RefCell<Option<String>>,
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        layout_requested: Cell<bool>,
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

impl Tree {
    fn intrinsic_size(&self) -> Size {
        let h = self.flat.len() as f32 * TREE_ROW_HEIGHT + self.search_height_for_intrinsic();
        Size::new(200.0, h.max(TREE_ROW_HEIGHT))
    }

    pub(crate) fn body_viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| (Self::normalized_frame(f).h - self.search_height_for_intrinsic()).max(0.0))
            .unwrap_or_else(|| (300.0 - self.search_height_for_intrinsic()).max(0.0))
    }

    fn search_height_for_intrinsic(&self) -> f32 {
        if self.searchable {
            32.0
        } else {
            0.0
        }
    }

    fn local_frame(&self) -> Rect {
        self.last_frame
            .get()
            .map(|frame| Self::normalized_frame(Rect::new(0.0, 0.0, frame.w, frame.h)))
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 200.0, 300.0))
    }

    fn action_at_point(&self, point: Point) -> Option<TreePointerAction> {
        let frame = self.local_frame();
        if !frame.contains(point) {
            return None;
        }
        if self.searchable && point.y < frame.y + self.search_height_for_intrinsic() {
            return None;
        }
        let content_y = point.y - frame.y - self.search_height_for_intrinsic()
            + self.body_scroll.scroll_offset();
        if content_y < 0.0 {
            return None;
        }
        let index = (content_y / TREE_ROW_HEIGHT) as usize;
        let node = self.flat.get(index)?;
        if node.disabled {
            return None;
        }
        let y = frame.y + self.search_height_for_intrinsic() + index as f32 * TREE_ROW_HEIGHT
            - self.body_scroll.scroll_offset();
        let geometry = self.row_geometry(frame, index, y);
        if geometry.check.is_some_and(|rect| rect.contains(point)) {
            return Some(TreePointerAction::Check(node.key.clone()));
        }
        if geometry.toggle.is_some_and(|rect| rect.contains(point)) {
            return Some(TreePointerAction::Toggle(node.key.clone()));
        }
        geometry
            .row
            .contains(point)
            .then(|| TreePointerAction::Select(node.key.clone()))
    }

    fn drop_position_at(&self, point: Point) -> DropPosition {
        let frame = self.local_frame();
        let content_y = point.y - frame.y - self.search_height_for_intrinsic()
            + self.body_scroll.scroll_offset();
        let index = (content_y / TREE_ROW_HEIGHT).max(0.0) as usize;
        let row_y = frame.y + self.search_height_for_intrinsic() + index as f32 * TREE_ROW_HEIGHT
            - self.body_scroll.scroll_offset();
        let relative = point.y - row_y;
        if relative < TREE_ROW_HEIGHT / 3.0 {
            DropPosition::Before
        } else if relative > TREE_ROW_HEIGHT * 2.0 / 3.0 {
            DropPosition::After
        } else if frame.contains(point) {
            DropPosition::Inside
        } else {
            DropPosition::After
        }
    }

    fn row_geometry(&self, frame: Rect, index: usize, y: f32) -> TreeRowGeometry {
        let node = &self.flat[index];
        let body_top = frame.y + self.search_height_for_intrinsic();
        let body_bottom = body_top + self.body_viewport_height();
        let visible_top = y.max(body_top);
        let visible_bottom = (y + TREE_ROW_HEIGHT).min(body_bottom);
        let row = Rect::new(
            frame.x,
            visible_top,
            frame.w,
            (visible_bottom - visible_top).max(0.0),
        );
        let reserved_slots = usize::from(node.checkable)
            + usize::from(node.has_children)
            + usize::from(!node.icon.is_empty());
        let max_indent =
            (frame.w - reserved_slots as f32 * TREE_SLOT_WIDTH - TREE_MIN_TITLE_WIDTH).max(0.0);
        let indent = (node.depth as f32 * TREE_INDENT_WIDTH).min(max_indent);
        let mut cursor = frame.x + indent;
        let mut take_slot = || {
            let width = TREE_SLOT_WIDTH.min((frame.x + frame.w - cursor).max(0.0));
            let slot = Rect::new(cursor, row.y, width, row.h);
            cursor += width;
            slot
        };
        let check = node
            .checkable
            .then(&mut take_slot)
            .filter(|rect| rect.w > 0.0);
        let toggle = node
            .has_children
            .then(&mut take_slot)
            .filter(|rect| rect.w > 0.0);
        let icon = (!node.icon.is_empty())
            .then(&mut take_slot)
            .filter(|rect| rect.w > 0.0);
        let title = Rect::new(cursor, row.y, (frame.x + frame.w - cursor).max(0.0), row.h);
        TreeRowGeometry {
            row,
            check,
            toggle,
            icon,
            title,
        }
    }

    fn normalized_frame(frame: Rect) -> Rect {
        Rect::new(
            frame.x,
            frame.y,
            if frame.w.is_finite() {
                frame.w.max(0.0)
            } else {
                0.0
            },
            if frame.h.is_finite() {
                frame.h.max(0.0)
            } else {
                0.0
            },
        )
    }

    fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        let Some(value) = Self::elide_single_line(ctx, value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size.min(frame.h * 0.65));
        ctx.pop_clip();
    }

    fn elide_single_line(
        ctx: &mut PaintContext,
        value: &str,
        font_size: f32,
        max_width: f32,
    ) -> Option<String> {
        if !max_width.is_finite() || max_width <= 0.0 {
            return None;
        }
        let value = value.replace(['\r', '\n'], " ");
        if Self::text_width(ctx, &value, font_size) <= max_width {
            return Some(value);
        }
        const ELLIPSIS: &str = "…";
        if Self::text_width(ctx, ELLIPSIS, font_size) > max_width {
            return None;
        }
        let mut visible = String::new();
        for ch in value.chars() {
            visible.push(ch);
            visible.push_str(ELLIPSIS);
            let fits = Self::text_width(ctx, &visible, font_size) <= max_width;
            visible.pop();
            if !fits {
                visible.pop();
                break;
            }
        }
        visible.push_str(ELLIPSIS);
        Some(visible)
    }

    fn text_width(ctx: &mut PaintContext, value: &str, font_size: f32) -> f32 {
        ctx.measure_text(value, font_size).w.max(
            crate::draw::resources::font::text_backend::estimate_text_metrics(
                value,
                f32::INFINITY,
                font_size,
            )
            .max_line_width,
        )
    }

    fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    pub fn new(nodes: Vec<TreeNode>) -> Self {
        let mut tree = Self {
            nodes,
            flat: Vec::new(),
            selected_key: String::new(),
            selected_keys: Vec::new(),
            expanded_keys: Vec::new(),
            multiple: false,
            searchable: false,
            search_query: String::new(),
            draggable: false,
            drop_callback: None,
            expand_callback: None,
            focused: false,
            hovered_action: None,
            pressed_action: None,
            dragged_key: None,
            pending_change: RefCell::new(None),
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
            layout_requested: Cell::new(false),
            last_frame: Cell::new(None),
        };
        tree.flatten();
        tree
    }

    pub fn selected_key(&self) -> &str {
        &self.selected_key
    }

    pub fn selected_keys(&self) -> &[String] {
        &self.selected_keys
    }

    pub fn set_selected_key(&mut self, key: &str) {
        self.selected_key = key.to_string();
        if !self.multiple {
            self.selected_keys.clear();
            if !key.is_empty() {
                self.selected_keys.push(key.to_string());
            }
        }
    }

    pub fn multiple(mut self, v: bool) -> Self {
        self.multiple = v;
        self
    }

    /// 在树顶显示可输入的搜索框，并按节点标题或 key 过滤可见行。
    pub fn searchable(mut self, v: bool) -> Self {
        self.searchable = v;
        if !v {
            self.search_query.clear();
        }
        self.flatten();
        self
    }

    /// 当前搜索关键字。
    pub fn search_query(&self) -> &str {
        &self.search_query
    }

    /// 直接替换搜索关键字并刷新可见节点。
    pub fn set_search_query(&mut self, query: impl Into<String>) {
        self.search_query = query.into();
        self.refresh_search();
    }

    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.refresh_search();
    }

    #[cfg(test)]
    pub(crate) fn visible_keys_for_test(&self) -> Vec<String> {
        self.flat.iter().map(|node| node.key.clone()).collect()
    }

    pub fn draggable(mut self, v: bool) -> Self {
        self.draggable = v;
        self
    }

    /// 用新子节点替换指定懒加载节点的子树。
    pub fn patch_children(&mut self, key: &str, children: Vec<TreeNode>) -> bool {
        let is_leaf = {
            let Some(node) = self.find_node_mut(key) else {
                return false;
            };
            node.children = children;
            node.lazy = false;
            node.is_leaf = node.children.is_empty();
            node.is_leaf
        };
        if is_leaf {
            self.expanded_keys.retain(|expanded| expanded != key);
        }
        self.flatten();
        self.layout_requested.set(true);
        true
    }

    /// 开启节点拖拽并注册放置回调。
    pub fn on_drop<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str, &str, DropPosition) + 'static,
    {
        self.drop_callback = Some(Rc::new(callback));
        self
    }

    /// 展开懒加载节点时通知应用层。
    pub fn on_expand<F>(mut self, callback: F) -> Self
    where
        F: Fn(&str) + 'static,
    {
        self.expand_callback = Some(Rc::new(callback));
        self
    }

    fn toggle_check(&mut self, key: &str) {
        if let Some(node) = self.find_node_mut(key) {
            node.checked = !node.checked;
            self.flatten();
        }
    }

    fn commit_pointer_action(&mut self, action: TreePointerAction) {
        match action {
            TreePointerAction::Check(key) => {
                self.toggle_check(&key);
                self.pending_change.replace(Some(key));
            }
            TreePointerAction::Toggle(key) => {
                let expanded = self
                    .flat
                    .iter()
                    .find(|node| node.key == key)
                    .is_some_and(|node| node.expanded);
                self.set_expanded(&key, !expanded);
            }
            TreePointerAction::Select(key) => self.select_from_pointer(key),
        }
    }

    fn select_from_pointer(&mut self, key: String) {
        self.selected_key.clone_from(&key);
        if self.multiple {
            if let Some(index) = self
                .selected_keys
                .iter()
                .position(|selected| *selected == key)
            {
                self.selected_keys.remove(index);
            } else {
                self.selected_keys.push(key.clone());
            }
        } else {
            self.selected_keys.clear();
            self.selected_keys.push(key.clone());
        }
        self.pending_change.replace(Some(key));
    }

    fn move_selection(&mut self, forward: bool) {
        let enabled = self
            .flat
            .iter()
            .enumerate()
            .filter_map(|(index, node)| (!node.disabled).then_some(index))
            .collect::<Vec<_>>();
        if enabled.is_empty() {
            return;
        }
        let current = enabled
            .iter()
            .position(|&index| self.flat[index].key == self.selected_key);
        let position = match (current, forward) {
            (Some(position), true) => (position + 1).min(enabled.len() - 1),
            (Some(position), false) => position.saturating_sub(1),
            (None, true) => 0,
            (None, false) => enabled.len() - 1,
        };
        self.focus_visible_index(enabled[position]);
    }

    fn focus_visible_index(&mut self, index: usize) {
        let key = self.flat[index].key.clone();
        let changed = key != self.selected_key;
        self.selected_key.clone_from(&key);
        if !self.multiple {
            self.selected_keys.clear();
            self.selected_keys.push(key.clone());
            if changed {
                self.pending_change.replace(Some(key));
            }
        }
        self.reveal_index(index);
    }

    fn expand_or_descend(&mut self) {
        let Some(index) = self.current_visible_index() else {
            self.move_selection(true);
            return;
        };
        let key = self.flat[index].key.clone();
        let depth = self.flat[index].depth;
        if !self.flat[index].has_children {
            return;
        }
        if !self.flat[index].expanded {
            self.set_expanded(&key, true);
            return;
        }
        let child = ((index + 1)..self.flat.len())
            .take_while(|&candidate| self.flat[candidate].depth > depth)
            .find(|&candidate| !self.flat[candidate].disabled);
        if let Some(child) = child {
            self.focus_visible_index(child);
        }
    }

    fn collapse_or_ascend(&mut self) {
        let Some(index) = self.current_visible_index() else {
            self.move_selection(false);
            return;
        };
        let key = self.flat[index].key.clone();
        let depth = self.flat[index].depth;
        if self.flat[index].has_children && self.flat[index].expanded {
            self.set_expanded(&key, false);
            return;
        }
        let parent = (0..index).rev().find(|&candidate| {
            self.flat[candidate].depth < depth && !self.flat[candidate].disabled
        });
        if let Some(parent) = parent {
            self.focus_visible_index(parent);
        }
    }

    fn activate_current(&mut self, prefer_check: bool) {
        let Some(index) = self.current_visible_index() else {
            self.move_selection(true);
            return;
        };
        let key = self.flat[index].key.clone();
        if prefer_check && self.flat[index].checkable {
            self.toggle_check(&key);
            self.pending_change.replace(Some(key));
        } else if self.multiple {
            self.select_from_pointer(key);
        }
    }

    fn current_visible_index(&self) -> Option<usize> {
        self.flat
            .iter()
            .position(|node| node.key == self.selected_key && !node.disabled)
    }

    fn set_expanded(&mut self, key: &str, expanded: bool) {
        let old_len = self.flat.len();
        let newly_expanded = if expanded {
            if self.expanded_keys.iter().any(|candidate| candidate == key) {
                false
            } else {
                self.expanded_keys.push(key.to_string());
                true
            }
        } else {
            self.expanded_keys.retain(|candidate| candidate != key);
            false
        };
        if newly_expanded {
            if let Some(node) = self.find_node_mut(key) {
                if node.lazy {
                    if let Some(callback) = self.expand_callback.as_ref() {
                        callback(key);
                    }
                }
            }
        }
        self.flatten();
        if self.flat.len() != old_len {
            self.layout_requested.set(true);
        }
        self.body_scroll.clamp_to_content(
            self.flat.len(),
            TREE_ROW_HEIGHT,
            self.body_viewport_height(),
        );
    }

    fn reveal_index(&mut self, index: usize) {
        let viewport_height = self.body_viewport_height();
        let old_offset = self.body_scroll.scroll_offset();
        let row_top = index as f32 * TREE_ROW_HEIGHT;
        let row_bottom = row_top + TREE_ROW_HEIGHT;
        let new_offset = if row_top < old_offset {
            row_top
        } else if row_bottom > old_offset + viewport_height {
            row_bottom - viewport_height
        } else {
            old_offset
        };
        self.body_scroll.set_scroll_offset(new_offset);
        self.body_scroll
            .clamp_to_content(self.flat.len(), TREE_ROW_HEIGHT, viewport_height);
        let applied = self.body_scroll.scroll_offset() - old_offset;
        if applied.abs() > 0.01 {
            self.push_scroll_delta(0.0, applied);
        }
    }

    fn find_node_mut(&mut self, key: &str) -> Option<&mut TreeNode> {
        Self::find_in_nodes(&mut self.nodes, key)
    }

    fn find_in_nodes<'a>(nodes: &'a mut [TreeNode], key: &str) -> Option<&'a mut TreeNode> {
        for node in nodes.iter_mut() {
            if node.key == key {
                return Some(node);
            }
            if let Some(found) = Self::find_in_nodes(&mut node.children, key) {
                return Some(found);
            }
        }
        None
    }

    fn flatten(&mut self) {
        let mut flat = Vec::new();
        let query = self.search_query.trim().to_lowercase();
        for node in &self.nodes {
            Self::flatten_node_filtered(node, 0, &self.expanded_keys, &query, &mut flat);
        }
        self.flat = flat;
    }

    fn refresh_search(&mut self) {
        self.flatten();
        self.body_scroll.clamp_to_content(
            self.flat.len(),
            TREE_ROW_HEIGHT,
            self.body_viewport_height(),
        );
        self.layout_requested.set(true);
    }

    fn flatten_node_filtered(
        node: &TreeNode,
        depth: usize,
        expanded_keys: &[String],
        query: &str,
        flat: &mut Vec<FlatNode>,
    ) -> bool {
        let matches = query.is_empty() || Self::node_matches(node, query);
        let descendant_matches = !query.is_empty()
            && node
                .children
                .iter()
                .any(|child| Self::node_matches_descendant(child, query));
        if !matches && !descendant_matches {
            return false;
        }

        let is_expanded = expanded_keys.contains(&node.key);
        let has_children = !node.children.is_empty() || node.lazy;
        flat.push(FlatNode {
            title: node.title.clone(),
            key: node.key.clone(),
            icon: node.icon.clone(),
            depth,
            has_children,
            expanded: is_expanded,
            disabled: node.disabled,
            checkable: node.checkable,
            checked: node.checked,
        });

        let show_children = if query.is_empty() {
            is_expanded
        } else {
            matches || descendant_matches
        };
        if show_children {
            for child in &node.children {
                Self::flatten_node_filtered(child, depth + 1, expanded_keys, query, flat);
            }
        }
        true
    }

    fn node_matches_descendant(node: &TreeNode, query: &str) -> bool {
        Self::node_matches(node, query)
            || node
                .children
                .iter()
                .any(|child| Self::node_matches_descendant(child, query))
    }

    fn node_matches(node: &TreeNode, query: &str) -> bool {
        if let Some(predicate) = node.filter.as_ref() {
            predicate(node, query)
        } else {
            node.title.to_lowercase().contains(query)
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let old_flat_len = self.flat.len();
        let old_searchable = self.searchable;
        let mut nodes = next.nodes;
        Self::preserve_checked_state(&self.nodes, &mut nodes);
        self.nodes = nodes;
        self.multiple = next.multiple;
        self.searchable = next.searchable;
        if !self.searchable {
            self.search_query.clear();
        }
        self.draggable = next.draggable;
        self.drop_callback = next.drop_callback;
        self.expand_callback = next.expand_callback;
        self.selected_key = if Self::contains_key(&self.nodes, &self.selected_key) {
            self.selected_key.clone()
        } else {
            String::new()
        };
        let nodes = &self.nodes;
        self.selected_keys
            .retain(|key| Self::contains_key(nodes, key));
        self.expanded_keys
            .retain(|key| Self::contains_key(nodes, key));
        self.hovered_action = None;
        self.pressed_action = None;
        self.dragged_key = None;
        self.flatten();
        if old_searchable != self.searchable || old_flat_len != self.flat.len() {
            self.layout_requested.set(true);
        }
        self.body_scroll.clamp_to_content(
            self.flat.len(),
            TREE_ROW_HEIGHT,
            self.body_viewport_height(),
        );
    }

    fn preserve_checked_state(old_nodes: &[TreeNode], new_nodes: &mut [TreeNode]) {
        for new_node in new_nodes {
            if let Some(old_node) = Self::find_in_nodes_ref(old_nodes, &new_node.key) {
                new_node.checked = old_node.checked;
            }
            Self::preserve_checked_state(old_nodes, &mut new_node.children);
        }
    }

    fn contains_key(nodes: &[TreeNode], key: &str) -> bool {
        !key.is_empty() && Self::find_in_nodes_ref(nodes, key).is_some()
    }

    fn find_in_nodes_ref<'a>(nodes: &'a [TreeNode], key: &str) -> Option<&'a TreeNode> {
        for node in nodes {
            if node.key == key {
                return Some(node);
            }
            if let Some(found) = Self::find_in_nodes_ref(&node.children, key) {
                return Some(found);
            }
        }
        None
    }

    pub(crate) fn snapshot_fields(&self) -> SnapshotFields {
        SnapshotFields::Tree {
            nodes: self
                .nodes
                .iter()
                .map(SnapshotTreeNode::from_tree_node)
                .collect(),
            selected_key: self.selected_key.clone(),
            selected_keys: self.selected_keys.clone(),
            expanded_keys: self.expanded_keys.clone(),
            multiple: self.multiple,
        }
    }
}

impl TreeNode {
    pub fn new(title: &str, key: &str) -> Self {
        Self {
            title: title.to_string(),
            key: key.to_string(),
            icon: String::new(),
            children: Vec::new(),
            disabled: false,
            checkable: false,
            checked: false,
            draggable: false,
            lazy: false,
            is_leaf: true,
            filter: None,
        }
    }

    pub fn icon(mut self, i: &str) -> Self {
        self.icon = i.to_string();
        self
    }

    pub fn children(mut self, c: Vec<TreeNode>) -> Self {
        self.children = c;
        self.is_leaf = false;
        self
    }

    #[allow(
        clippy::should_implement_trait,
        reason = "add is the established fluent builder API, not arithmetic addition"
    )]
    pub fn add(mut self, child: TreeNode) -> Self {
        self.children.push(child);
        self.is_leaf = false;
        self
    }

    pub fn disabled(mut self, v: bool) -> Self {
        self.disabled = v;
        self
    }

    pub fn checkable(mut self, v: bool) -> Self {
        self.checkable = v;
        self
    }

    pub fn draggable(mut self, v: bool) -> Self {
        self.draggable = v;
        self
    }

    /// 标记为展开时由应用异步补充子节点的节点。
    pub fn lazy(mut self, v: bool) -> Self {
        self.lazy = v;
        self
    }

    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&Self, &str) -> bool + 'static,
    {
        self.filter = Some(Rc::new(predicate));
        self
    }
}
