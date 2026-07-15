use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::{
    ComponentId, EventResult, KeyCode, MouseButton, SemanticEvent, SnapshotFields,
    SnapshotTreeNode, SystemEvent, WidgetTree,
};
use std::cell::{Cell, RefCell};

pub(crate) const TREE_ROW_HEIGHT: f32 = 28.0;

#[derive(Debug, Clone, PartialEq)]
pub struct TreeNode {
    pub title: String,
    pub key: String,
    pub icon: String,
    pub children: Vec<TreeNode>,
    pub disabled: bool,
    pub checkable: bool,
    pub checked: bool,
    pub draggable: bool,
    pub is_leaf: bool,
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
        focused: bool,
        pending_change: RefCell<Option<String>>,
        pub(crate) body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        pub(crate) last_frame: Cell<Option<Rect>>,
    }

    tab_index => (&self) -> i32 { 1 }

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

    on_event => (&mut self, event: &SystemEvent) -> EventResult {
        match event {
            SystemEvent::Wheel { delta, .. } => {
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
                if let Some(idx) = self.row_index_at_y(pos.y) {
                    let node_key = self.flat[idx].key.clone();
                    let node_disabled = self.flat[idx].disabled;

                    if node_disabled {
                        return EventResult::NotHandled;
                    }

                    let indent = self.flat[idx].depth as f32 * 20.0;

                    let check_x = indent;
                    if self.flat[idx].checkable && pos.x >= check_x && pos.x < check_x + 20.0 {
                        self.toggle_check(&node_key);
                        self.pending_change.replace(Some(node_key));
                        return EventResult::Handled;
                    }

                    let arrow_x = indent + 20.0;
                    if pos.x >= arrow_x && pos.x < arrow_x + 20.0 && self.flat[idx].has_children {
                        self.set_expanded(&node_key, !self.flat[idx].expanded);
                        return EventResult::Handled;
                    }

                    self.select_from_pointer(node_key);
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            SystemEvent::FocusIn => {
                self.focused = true;
                EventResult::Handled
            }
            SystemEvent::FocusOut => {
                self.focused = false;
                EventResult::Handled
            }
            SystemEvent::KeyDown { key, .. } => match key {
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
            _ => EventResult::NotHandled,
        }
    }

    semantic_event => (&self, id: ComponentId, _event: &SystemEvent) -> Option<SemanticEvent> {
        self.pending_change
            .borrow_mut()
            .take()
            .map(|key| SemanticEvent::change(id, key))
    }

    render => (&self, frame: Rect, ctx: &mut PaintContext, _tree: &WidgetTree) {
        self.last_frame.set(Some(frame));
        let primary = ctx.tokens().color_primary();
        let text = ctx.tokens().color_text();
        let text_sec = ctx.tokens().color_text_secondary();
        let fill = ctx.tokens().color_fill_tertiary();

        let viewport_h = self.body_viewport_height();
        let (start, end) = self
            .body_scroll
            .scroll_range(self.flat.len(), TREE_ROW_HEIGHT, viewport_h);
        ctx.canvas_2d().push_clip(frame);

        for i in start..end {
            let node = &self.flat[i];
            let y = frame.y + i as f32 * TREE_ROW_HEIGHT - self.body_scroll.scroll_offset();
            if y + TREE_ROW_HEIGHT < frame.y || y > frame.y + viewport_h {
                continue;
            }
            let indent = node.depth as f32 * 20.0;
            let is_selected = self.multiple && self.selected_keys.contains(&node.key)
                || (!self.multiple && node.key == self.selected_key);

            if is_selected {
                ctx.fill_rect(Rect::new(frame.x, y, frame.w, TREE_ROW_HEIGHT), fill, None);
            }

            let row_rect = Rect::new(frame.x, y, frame.w, TREE_ROW_HEIGHT);
            if self.focused && self.multiple && node.key == self.selected_key {
                ctx.stroke_rect(row_rect, primary, 1.0, None);
            }
            let mut cursor = frame.x + indent;

            if node.checkable {
                let check_str = if node.checked { "[x]" } else { "[ ]" };
                let check_y = ctx.visual_center_y(row_rect, 12.0);
                ctx.draw_text(
                    check_str,
                    Point::new(cursor + 2.0, check_y),
                    if node.checked { primary } else { text_sec },
                    12.0,
                );
                cursor += 28.0;
            }

            let row_y = ctx.visual_center_y(row_rect, 10.0);
            if node.has_children {
                let arrow = if node.expanded { "v" } else { ">" };
                ctx.draw_text(arrow, Point::new(cursor + 4.0, row_y), text_sec, 10.0);
            }
            cursor += 20.0;

            if !node.icon.is_empty() {
                let icon_rect = Rect::new(cursor, row_rect.y, 16.0, row_rect.h);
                crate::ui::widgets::icon::paint_icon_in_frame(
                    ctx, &node.icon, icon_rect, text_sec, 12.0,
                );
                cursor += 20.0;
            }

            let tc = if node.disabled {
                text_sec
            } else if is_selected {
                primary
            } else {
                text
            };
            let title_y = ctx.visual_center_y(row_rect, 13.0);
            ctx.draw_text(&node.title, Point::new(cursor, title_y), tc, 13.0);
        }

        ctx.canvas_2d().pop_clip();
        if self.focused {
            ctx.stroke_rect(frame, primary, 1.5, None);
        }
    }
}

impl Tree {
    fn intrinsic_size(&self) -> Size {
        let h = self.flat.len() as f32 * TREE_ROW_HEIGHT;
        Size::new(200.0, h.max(TREE_ROW_HEIGHT))
    }

    pub(crate) fn body_viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| f.h.max(TREE_ROW_HEIGHT))
            .unwrap_or(300.0)
    }

    fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let local_y = pos_y + self.body_scroll.scroll_offset();
        if local_y < 0.0 {
            return None;
        }
        let idx = (local_y / TREE_ROW_HEIGHT) as usize;
        if idx < self.flat.len() {
            Some(idx)
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

    pub fn new(nodes: Vec<TreeNode>) -> Self {
        let mut tree = Self {
            nodes,
            flat: Vec::new(),
            selected_key: String::new(),
            selected_keys: Vec::new(),
            expanded_keys: Vec::new(),
            multiple: false,
            focused: false,
            pending_change: RefCell::new(None),
            body_scroll: VirtualListScroll::new(),
            scroll_delta_strip: Cell::new((0.0, 0.0)),
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

    fn toggle_check(&mut self, key: &str) {
        if let Some(node) = self.find_node_mut(key) {
            node.checked = !node.checked;
            self.flatten();
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
        if expanded {
            if !self.expanded_keys.iter().any(|candidate| candidate == key) {
                self.expanded_keys.push(key.to_string());
            }
        } else {
            self.expanded_keys.retain(|candidate| candidate != key);
        }
        self.flatten();
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
        self.flat.clear();
        let nodes = self.nodes.clone();
        for node in &nodes {
            self.flatten_node(node, 0);
        }
    }

    fn flatten_node(&mut self, node: &TreeNode, depth: usize) {
        let is_expanded = self.expanded_keys.contains(&node.key);
        let has_children = !node.children.is_empty();
        self.flat.push(FlatNode {
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
        if is_expanded {
            for child in &node.children {
                self.flatten_node(child, depth + 1);
            }
        }
    }

    pub(crate) fn sync_from(&mut self, next: Self) {
        let mut nodes = next.nodes;
        Self::preserve_checked_state(&self.nodes, &mut nodes);
        self.nodes = nodes;
        self.multiple = next.multiple;
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
        self.flatten();
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
            is_leaf: true,
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
}
