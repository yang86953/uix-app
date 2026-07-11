use crate::component;
use crate::core::{Constraints, Point, Rect, Size};
use crate::draw::painting::PaintContext;
use crate::ui::foundation::virtual_scroll::VirtualListScroll;
use crate::ui::{EventResult, SnapshotFields, SnapshotTreeNode, SystemEvent, WidgetTree};
use std::cell::Cell;

const TREE_ROW_HEIGHT: f32 = 28.0;

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

struct FlatNode {
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
        flat: Vec<FlatNode>,
        selected_key: String,
        selected_keys: Vec<String>,
        expanded_keys: Vec<String>,
        multiple: bool,
        body_scroll: VirtualListScroll,
        scroll_delta_strip: Cell<(f32, f32)>,
        last_frame: Cell<Option<Rect>>,
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
            SystemEvent::PointerDown { pos, .. } => {
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
                        return EventResult::Handled;
                    }

                    let arrow_x = indent + 20.0;
                    if pos.x >= arrow_x && pos.x < arrow_x + 20.0 && self.flat[idx].has_children {
                        if let Some(ek_idx) = self.expanded_keys.iter().position(|k| *k == node_key) {
                            self.expanded_keys.remove(ek_idx);
                        } else {
                            self.expanded_keys.push(node_key.clone());
                        }
                        self.flatten();
                        self.body_scroll.clamp_to_content(
                            self.flat.len(),
                            TREE_ROW_HEIGHT,
                            self.body_viewport_height(),
                        );
                        return EventResult::Handled;
                    }

                    if self.multiple {
                        if let Some(ex_idx) = self.selected_keys.iter().position(|k| *k == node_key) {
                            self.selected_keys.remove(ex_idx);
                        } else {
                            self.selected_keys.push(node_key.clone());
                        }
                    } else {
                        self.selected_key = node_key.clone();
                        self.selected_keys.clear();
                        self.selected_keys.push(node_key);
                    }
                    return EventResult::Handled;
                }
                EventResult::NotHandled
            }
            _ => EventResult::NotHandled,
        }
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
    }
}

impl Tree {
    fn intrinsic_size(&self) -> Size {
        let h = self.flat.len() as f32 * TREE_ROW_HEIGHT;
        Size::new(200.0, h.max(TREE_ROW_HEIGHT))
    }

    fn body_viewport_height(&self) -> f32 {
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

#[cfg(test)]
#[path = "../../../tests/ui/widgets/display/tree_virtual_scroll.rs"]
mod tests;
