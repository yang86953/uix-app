use crate::core::{Point, Rect, Size};
use crate::ui::widget_runtime::paint_context::PaintContext;
use crate::ui::virtualization::virtual_scroll::VirtualListScroll;
use crate::ui::{SnapshotFields, SnapshotTreeNode};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use super::component::FlatNode;
use super::{
    DropPosition, TREE_INDENT_WIDTH, TREE_MIN_TITLE_WIDTH, TREE_ROW_HEIGHT, TREE_SLOT_WIDTH, Tree,
    TreeNode, TreePointerAction, TreeRowGeometry,
};

impl Tree {
    pub(crate) fn intrinsic_size(&self) -> Size {
        let h = self.flat.len() as f32 * TREE_ROW_HEIGHT + self.search_height_for_intrinsic();
        Size::new(200.0, h.max(TREE_ROW_HEIGHT))
    }

    pub(crate) fn body_viewport_height(&self) -> f32 {
        self.last_frame
            .get()
            .map(|f| (Self::normalized_frame(f).h - self.search_height_for_intrinsic()).max(0.0))
            .unwrap_or_else(|| (300.0 - self.search_height_for_intrinsic()).max(0.0))
    }

    pub(crate) fn search_height_for_intrinsic(&self) -> f32 {
        if self.searchable { 32.0 } else { 0.0 }
    }

    pub(crate) fn local_frame(&self) -> Rect {
        self.last_frame
            .get()
            .map(|frame| Self::normalized_frame(Rect::new(0.0, 0.0, frame.w, frame.h)))
            .unwrap_or_else(|| Rect::new(0.0, 0.0, 200.0, 300.0))
    }

    pub(crate) fn action_at_point(&self, point: Point) -> Option<TreePointerAction> {
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

    pub(crate) fn drop_position_at(&self, point: Point) -> DropPosition {
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

    pub(crate) fn row_geometry(&self, frame: Rect, index: usize, y: f32) -> TreeRowGeometry {
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

    pub(crate) fn normalized_frame(frame: Rect) -> Rect {
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

    pub(crate) fn paint_single_line(
        ctx: &mut PaintContext,
        value: &str,
        frame: Rect,
        color: crate::draw::Color,
        font_size: f32,
    ) {
        if frame.w <= 0.0 || frame.h <= 0.0 {
            return;
        }
        // 复用 UI 绘制上下文拥有的保守单行省略算法。
        let Some(value) = ctx.elide_single_line(value, font_size, frame.w) else {
            return;
        };
        ctx.push_clip(frame);
        ctx.draw_text_in_frame(&value, frame, color, font_size.min(frame.h * 0.65));
        ctx.pop_clip();
    }

    pub(crate) fn push_scroll_delta(&self, dx: f32, dy: f32) {
        if dx.abs() <= 0.01 && dy.abs() <= 0.01 {
            return;
        }
        let current = self.scroll_delta_strip.get();
        self.scroll_delta_strip
            .set((current.0 + dx, current.1 + dy));
    }

    /// 创建持有完整节点树、无选择且默认折叠的树组件。
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

    /// 为整棵树统一启用或禁用勾选入口。
    pub fn checkable(mut self, value: bool) -> Self {
        // 把树级文档配置递归投影到每个节点。
        Self::set_nodes_checkable(&mut self.nodes, value);
        // 节点勾选槽位变化后立即刷新扁平渲染快照。
        self.flatten();
        // 返回可继续组合的树组件。
        self
    }

    /// 设置首次物化时是否展开全部可展开节点。
    pub fn default_expand_all(mut self, value: bool) -> Self {
        // 默认展开只构造初始键集合，不成为受控运行态。
        let mut expanded_keys = Vec::new();
        // 启用时递归收集拥有子节点或懒加载能力的稳定键。
        if value {
            // 从完整节点树生成初始展开集合。
            Self::collect_expandable_keys(&self.nodes, &mut expanded_keys);
        }
        // 用新初始配置替换构造期间的空集合。
        self.expanded_keys = expanded_keys;
        // 初始展开集合变化后重新生成可见行。
        self.flatten();
        // 返回可继续组合的树组件。
        self
    }

    // 递归应用树级勾选能力。
    fn set_nodes_checkable(nodes: &mut [TreeNode], value: bool) {
        // 遍历当前层的每个节点。
        for node in nodes {
            // 用树级配置覆盖节点自身的初始勾选入口。
            node.checkable = value;
            // 对完整子树应用相同配置。
            Self::set_nodes_checkable(&mut node.children, value);
        }
    }

    // 递归收集可展开节点的稳定键。
    fn collect_expandable_keys(nodes: &[TreeNode], expanded_keys: &mut Vec<String>) {
        // 按声明顺序遍历当前层节点。
        for node in nodes {
            // 有静态子节点或懒加载能力的节点都具有展开语义。
            if !node.children.is_empty() || node.lazy {
                // 保存稳定业务键而不是可见索引。
                expanded_keys.push(node.key.clone());
            }
            // 继续收集嵌套分支。
            Self::collect_expandable_keys(&node.children, expanded_keys);
        }
    }

    /// 返回当前主选择节点的稳定键；无选择时为空字符串。
    pub fn selected_key(&self) -> &str {
        &self.selected_key
    }

    /// 返回当前多选集合中的稳定节点键。
    pub fn selected_keys(&self) -> &[String] {
        &self.selected_keys
    }

    /// 设置主选择键；单选模式下同时替换选择集合。
    pub fn set_selected_key(&mut self, key: &str) {
        self.selected_key = key.to_string();
        if !self.multiple {
            self.selected_keys.clear();
            if !key.is_empty() {
                self.selected_keys.push(key.to_string());
            }
        }
    }

    /// 设置是否允许节点多选。
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

    /// 清空搜索关键字并恢复按展开状态计算的可见节点。
    pub fn clear_search(&mut self) {
        self.search_query.clear();
        self.refresh_search();
    }

    // 测试目标保留树可见 key 观测入口，供树过滤与展开测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn visible_keys_for_test(&self) -> Vec<String> {
        self.flat.iter().map(|node| node.key.clone()).collect()
    }

    /// 设置是否允许拖拽节点。
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

    pub(crate) fn toggle_check(&mut self, key: &str) {
        if let Some(node) = self.find_node_mut(key) {
            node.checked = !node.checked;
            self.flatten();
        }
    }

    pub(crate) fn commit_pointer_action(&mut self, action: TreePointerAction) {
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

    pub(crate) fn select_from_pointer(&mut self, key: String) {
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

    pub(crate) fn move_selection(&mut self, forward: bool) {
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

    pub(crate) fn focus_visible_index(&mut self, index: usize) {
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

    pub(crate) fn expand_or_descend(&mut self) {
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

    pub(crate) fn collapse_or_ascend(&mut self) {
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

    pub(crate) fn activate_current(&mut self, prefer_check: bool) {
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

    pub(crate) fn current_visible_index(&self) -> Option<usize> {
        self.flat
            .iter()
            .position(|node| node.key == self.selected_key && !node.disabled)
    }

    pub(crate) fn set_expanded(&mut self, key: &str, expanded: bool) {
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

    pub(crate) fn reveal_index(&mut self, index: usize) {
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

    pub(crate) fn find_node_mut(&mut self, key: &str) -> Option<&mut TreeNode> {
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

    pub(crate) fn flatten(&mut self) {
        let mut flat = Vec::new();
        let query = self.search_query.trim().to_lowercase();
        for node in &self.nodes {
            Self::flatten_node_filtered(node, 0, &self.expanded_keys, &query, &mut flat);
        }
        self.flat = flat;
    }

    pub(crate) fn refresh_search(&mut self) {
        self.flatten();
        self.body_scroll.clamp_to_content(
            self.flat.len(),
            TREE_ROW_HEIGHT,
            self.body_viewport_height(),
        );
        self.layout_requested.set(true);
    }

    pub(crate) fn flatten_node_filtered(
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

    pub(crate) fn node_matches_descendant(node: &TreeNode, query: &str) -> bool {
        Self::node_matches(node, query)
            || node
                .children
                .iter()
                .any(|child| Self::node_matches_descendant(child, query))
    }

    pub(crate) fn node_matches(node: &TreeNode, query: &str) -> bool {
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

    pub(crate) fn preserve_checked_state(old_nodes: &[TreeNode], new_nodes: &mut [TreeNode]) {
        for new_node in new_nodes {
            if let Some(old_node) = Self::find_in_nodes_ref(old_nodes, &new_node.key) {
                new_node.checked = old_node.checked;
            }
            Self::preserve_checked_state(old_nodes, &mut new_node.children);
        }
    }

    pub(crate) fn contains_key(nodes: &[TreeNode], key: &str) -> bool {
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
