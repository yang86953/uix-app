use super::*;
use crate::core::{Point, Rect};
use crate::draw::pipeline::InvalidationQueueHandle;
use crate::native::traits::input::{KeyMod, MouseButton};
use crate::ui::event::HandlerTable;
use crate::ui::overlay::OverlayStack;

#[path = "tree_layout.rs"]
mod tree_layout;

#[derive(Clone)]
pub(crate) struct DragGestureState {
    pub potential: bool,
    pub active: bool,
    pub start_pos: Point,
    pub last_pos: Point,
    pub button: MouseButton,
    pub mods: KeyMod,
    pub target: Option<WidgetId>,
}

impl Default for DragGestureState {
    fn default() -> Self {
        Self {
            potential: false,
            active: false,
            start_pos: Point::default(),
            last_pos: Point::default(),
            button: MouseButton::None,
            mods: KeyMod::NONE,
            target: None,
        }
    }
}

impl DragGestureState {
    pub fn reset(&mut self) {
        self.potential = false;
        self.active = false;
        self.button = MouseButton::None;
        self.mods = KeyMod::NONE;
        self.target = None;
    }
}

pub struct WidgetTree {
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_ids: Vec<WidgetId>,
    pub(crate) next_id: WidgetId,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) focused_widget: Option<WidgetId>,
    pub(crate) hovered_widget: Option<WidgetId>,
    pub(crate) scroll_region_move: Option<(Rect, f32, f32)>,
    pub(crate) pointer_down_target: Option<WidgetId>,
    pub tree_version: u64,
    cached_traversal: std::cell::RefCell<(Vec<WidgetId>, u64)>,

    pub(crate) handler_table: HandlerTable,
    pub(crate) overlay_stack: OverlayStack,

    pub(crate) drag_gesture: DragGestureState,
    pub(crate) invalidation: InvalidationQueueHandle,
    pub(crate) effects: Vec<crate::ui::foundation::state::Effect>,
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            free_ids: Vec::new(),
            next_id: 0,
            root_id: None,
            focused_widget: None,
            hovered_widget: None,
            scroll_region_move: None,
            pointer_down_target: None,
            tree_version: 0,
            cached_traversal: std::cell::RefCell::new((Vec::new(), 0)),
            handler_table: HandlerTable::new(),
            overlay_stack: OverlayStack::new(),
            drag_gesture: DragGestureState::default(),
            invalidation: crate::draw::pipeline::InvalidationQueue::shared(),
            effects: Vec::new(),
        }
    }
}

impl WidgetTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn tree_version(&self) -> u64 {
        self.tree_version
    }

    pub fn alloc_id(&mut self) -> WidgetId {
        if let Some(id) = self.free_ids.pop() {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn set_root_with_children(
        &mut self,
        widget: Box<dyn WidgetComponent>,
        children: Vec<Box<dyn WidgetComponent>>,
    ) -> WidgetId {
        let id = self.set_root(widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    pub fn add_child_with_children(
        &mut self,
        parent_id: WidgetId,
        widget: Box<dyn WidgetComponent>,
        children: Vec<Box<dyn WidgetComponent>>,
    ) -> WidgetId {
        let id = self.add_child(parent_id, widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    fn reset_interaction_state(&mut self) {
        self.focused_widget = None;
        self.hovered_widget = None;
        self.pointer_down_target = None;
    }

    fn collect_lifecycle_subtree(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
        if self.get(id).is_none() {
            return;
        }
        out.push(id);
        if let Some(node) = self.get(id) {
            for &child_id in node.children() {
                self.collect_lifecycle_subtree(child_id, out);
            }
        }
    }

    fn attach_node(&mut self, id: WidgetId) {
        if let Some(node) = self.get_mut(id) {
            if !node.attached() {
                node.set_attached(true);
                node.on_attach();
            }
        }
    }

    fn deactivate_detach_and_destroy(&mut self, id: WidgetId) {
        if let Some(node) = self.get_mut(id) {
            if node.active() {
                node.set_active(false);
                node.on_inactive();
            }
            if node.mounted() {
                node.set_mounted(false);
                node.on_unmount();
            }
            if node.attached() {
                node.set_attached(false);
                node.on_detach();
            }
            if !node.destroyed() {
                node.set_destroyed(true);
                node.on_destroy();
            }
        }
    }

    pub(crate) fn teardown_subtree(&mut self, id: WidgetId) {
        let mut ids = Vec::new();
        self.collect_lifecycle_subtree(id, &mut ids);
        for id in ids.into_iter().rev() {
            self.deactivate_detach_and_destroy(id);
        }
    }

    fn teardown_all(&mut self) {
        let ids = self.traverse();
        for id in ids.into_iter().rev() {
            self.deactivate_detach_and_destroy(id);
        }
    }

    pub fn notify_theme_changed(&mut self) {
        let ids = self.traverse();
        for id in ids {
            if let Some(node) = self.get_mut(id) {
                node.on_theme_changed();
            }
            if self.get(id).is_some_and(|node| node.uses_palette()) {
                self.invalidate_paint(id);
            }
        }
    }

    pub fn set_root(&mut self, widget: Box<dyn WidgetComponent>) -> WidgetId {
        self.teardown_all();

        // 纭噸缃細娓呯┖鏃ф爲锛孖D 绌洪棿褰掗浂锛宖ree_ids 搴熷純
        self.nodes.clear();
        self.free_ids.clear();
        self.next_id = 0;
        self.root_id = None;
        self.handler_table.clear();
        self.overlay_stack.clear();
        self.reset_interaction_state();
        self.tree_version += 1;

        let children = widget.build();
        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new(widget);
        boxed.set_id(id);
        let ps = boxed.preferred_size(None);
        boxed.set_frame(Rect::new(0.0, 0.0, ps.w, ps.h));
        if self.nodes.len() <= id {
            self.nodes.resize_with(id + 1, || None);
        }
        self.nodes[id] = Some(boxed);
        self.root_id = Some(id);
        self.attach_node(id);
        for child in children {
            self.add_child(id, child);
        }
        self.push_layout_invalidation(id);
        id
    }

    pub fn root(&self) -> Option<&BoxedWidget> {
        self.root_id
            .and_then(|id| self.nodes.get(id))
            .and_then(|n| n.as_ref())
    }
    pub fn root_id(&self) -> Option<WidgetId> {
        self.root_id
    }
    pub fn root_mut(&mut self) -> Option<&mut BoxedWidget> {
        self.root_id
            .and_then(|id| self.nodes.get_mut(id))
            .and_then(|n| n.as_mut())
    }

    pub fn find_by_type<T: WidgetComponent + 'static>(&self) -> Option<WidgetId> {
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.component().as_any().downcast_ref::<T>().is_some() {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn find_all_by_type<T: WidgetComponent + 'static>(&self) -> Vec<(WidgetId, &T)> {
        let mut results = Vec::new();
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if let Some(w) = node.component().as_any().downcast_ref::<T>() {
                    results.push((id, w));
                }
            }
        }
        results
    }

    pub fn find_by_type_and_modify<T: WidgetComponent + 'static>(
        &mut self,
        f: impl FnOnce(&mut T),
    ) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        if let Some(node) = self.get_mut(id) {
            if let Some(w) = node.component_mut().as_any_mut().downcast_mut::<T>() {
                f(w);
            }
        }
        Some(id)
    }

    pub fn get(&self, id: WidgetId) -> Option<&BoxedWidget> {
        self.nodes.get(id).and_then(|n| n.as_ref())
    }
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut BoxedWidget> {
        self.nodes.get_mut(id).and_then(|n| n.as_mut())
    }

    pub fn set_z_index(&mut self, id: WidgetId, z: i32) -> &mut Self {
        if let Some(n) = self.get_mut(id) {
            n.set_z_index(z);
        }
        self
    }

    pub fn add_child(&mut self, parent_id: WidgetId, child: Box<dyn WidgetComponent>) -> WidgetId {
        self.tree_version += 1;
        let children = child.build();
        let child_id = self.alloc_id();
        let mut boxed = BoxedWidget::new(child);
        boxed.set_id(child_id);
        boxed.set_parent(Some(parent_id));
        if self.nodes.len() <= child_id {
            self.nodes.resize_with(child_id + 1, || None);
        }
        self.nodes[child_id] = Some(boxed);
        self.attach_node(child_id);
        if let Some(parent) = self.get_mut(parent_id) {
            parent.children_mut().push(child_id);
        }
        for child in children {
            self.add_child(child_id, child);
        }

        // 结构变化：Layout 失效向上传播。
        self.push_layout_invalidation(parent_id);
        self.propagate_layout_invalidation(parent_id);

        child_id
    }

    pub fn remove(&mut self, id: WidgetId) {
        self.tree_version += 1;

        let old_frame = self
            .get(id)
            .map(|n| n.frame())
            .filter(|f| f.w > 0.0 && f.h > 0.0);

        let parent_id = self
            .nodes
            .get(id)
            .and_then(|n| n.as_ref())
            .and_then(|n| n.parent());
        self.teardown_subtree(id);
        if let Some(node) = self.nodes.get_mut(id) {
            if let Some(node) = node.take() {
                for child_id in node.children().to_vec() {
                    self.remove(child_id);
                }
                self.handler_table.clear_component(id);
                self.overlay_stack.remove_for_owner(id);
                self.free_ids.push(id);
            }
        }
        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_mut(pid) {
                parent.children_mut().retain(|&c| c != id);
            }
        }

        if let Some(frame) = old_frame {
            if let Some(pid) = parent_id {
                self.invalidate_paint_rect(pid, frame);
            }
        }
        if let Some(pid) = parent_id {
            self.push_layout_invalidation(pid);
            self.propagate_layout_invalidation(pid);
        }
    }

    pub fn set_visible(&mut self, id: WidgetId, visible: bool) {
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let children: Vec<WidgetId> = self
                .get(current)
                .map(|n| n.children().to_vec())
                .unwrap_or_default();

            let mut changed = false;
            if let Some(n) = self.get_mut(current) {
                if n.visible() != visible {
                    n.set_visible(visible);
                    self.tree_version += 1;
                    changed = true;
                }
            }

            if changed {
                self.invalidate_paint(current);
                self.push_layout_invalidation(current);
            }

            for child in children {
                stack.push(child);
            }
        }

        self.propagate_layout_invalidation(id);
        self.reconcile_lifecycle_after_layout();
    }

    pub fn traverse(&self) -> Vec<WidgetId> {
        let mut cache = self.cached_traversal.borrow_mut();
        let (ref mut ids, ref mut ver) = *cache;
        if *ver != self.tree_version {
            ids.clear();
            if let Some(root_id) = self.root_id {
                // 杩唬閬嶅巻锛堥伩鍏嶉€掑綊杩囨繁鏃剁殑鏍堟孩鍑猴級
                let mut stack = vec![root_id];
                while let Some(current) = stack.pop() {
                    ids.push(current);
                    if let Some(node) = self.get(current) {
                        for child_id in node.children().iter().rev() {
                            stack.push(*child_id);
                        }
                    }
                }
            }
            *ver = self.tree_version;
        }
        ids.clone()
    }

    pub fn set_frame_dirty(&mut self, id: WidgetId, new_frame: Rect) {
        let old = match self.get(id) {
            Some(w) => {
                let old = w.frame();
                if old == new_frame {
                    return;
                }
                old
            }
            None => return,
        };
        if let Some(w) = self.get_mut(id) {
            w.set_frame(new_frame);
        }
        self.invalidate_paint_rect(id, old);
        self.invalidate_paint(id);
        self.push_layout_invalidation(id);
        self.propagate_layout_invalidation(id);
    }

    // 鈹€鈹€ WidgetNode tree building 鈹€鈹€

    pub fn build(&mut self, node: WidgetNode) -> WidgetId {
        self.build_node(node, None)
    }

    pub(crate) fn build_child_node(&mut self, parent_id: WidgetId, node: WidgetNode) -> WidgetId {
        self.build_node(node, Some(parent_id))
    }

    pub fn set_children(&mut self, parent_id: WidgetId, children: Vec<WidgetNode>) {
        let old_children: Vec<WidgetId> = self
            .get(parent_id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        for &cid in &old_children {
            self.remove(cid);
        }
        self.tree_version += 1;
        for child in children {
            self.build_node(child, Some(parent_id));
        }
    }

    fn build_node(&mut self, node: WidgetNode, parent: Option<WidgetId>) -> WidgetId {
        let WidgetNode {
            widget,
            children,
            z_index,
            key,
            tab_idx,
            handlers,
        } = node;
        let id = match parent {
            Some(p) => self.add_child(p, widget),
            None => self.set_root(widget),
        };
        if let Some(n) = self.get_mut(id) {
            n.set_key(key);
            n.set_z_index(z_index);
            let ti = if tab_idx != 0 {
                tab_idx
            } else {
                n.component().tab_index()
            };
            n.set_tab_index(ti);
        }
        for handler in handlers {
            self.handler_table.register(id, handler);
        }
        for child in children {
            self.build_node(child, Some(id));
        }
        id
    }

    pub fn focus_by_type<T: WidgetComponent + 'static>(&mut self) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        self.focused_widget = Some(id);
        if let Some(node) = self.get_mut(id) {
            if let Some(input) = node
                .component_mut()
                .as_any_mut()
                .downcast_mut::<crate::ui::widgets::Input>()
            {
                input.set_focused(true);
            }
        }
        self.reconcile_lifecycle_after_layout();
        Some(id)
    }

    pub fn is_focused_type<T: WidgetComponent + 'static>(&self) -> bool {
        self.focused_widget
            .and_then(|id| self.get(id))
            .map(|node| node.component().as_any().downcast_ref::<T>().is_some())
            .unwrap_or(false)
    }

    pub fn handler_table(&mut self) -> &mut HandlerTable {
        &mut self.handler_table
    }

    pub fn overlay_stack(&self) -> &OverlayStack {
        &self.overlay_stack
    }

    pub fn overlay_stack_mut(&mut self) -> &mut OverlayStack {
        &mut self.overlay_stack
    }

    // 鈹€鈹€ Tab 閿劍鐐瑰鑸?鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€鈹€

    pub fn collect_focusable(&self) -> Vec<WidgetId> {
        let mut result: Vec<(i32, WidgetId)> = Vec::new();
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.is_focusable() {
                    result.push((node.tab_index(), id));
                }
            }
        }
        // 鎸?tab_index 鍗囧簭鎺掑簭锛堝皬鏁板瓧鍏堣仛鐒︼級
        result.sort_by_key(|&(idx, _)| idx);
        result.into_iter().map(|(_, id)| id).collect()
    }

    pub fn focus_next(&self, forward: bool) -> Option<WidgetId> {
        let focusable = self.collect_focusable();
        if focusable.is_empty() {
            return None;
        }
        let current = self.focused_widget;
        if let Some(cur_id) = current {
            let pos = focusable.iter().position(|&id| id == cur_id);
            match pos {
                Some(p) => {
                    if forward {
                        Some(focusable[(p + 1) % focusable.len()])
                    } else {
                        Some(focusable[(p + focusable.len() - 1) % focusable.len()])
                    }
                }
                None => Some(focusable[0]),
            }
        } else {
            Some(focusable[0])
        }
    }
}

#[cfg(test)]
#[path = "tree_core_tests.rs"]
mod tests;
