use super::*;
use crate::core::{Constraints, Rect, Size};
use crate::draw::pipeline::InvalidationQueueHandle;
use crate::ui::app_state::AppState;
use crate::ui::component_snapshot::ComponentConfigSnapshot;
use crate::ui::event::HandlerTable;
use crate::ui::foundation::focus_trap::next_focus_in_order;
use crate::ui::managers::WidgetManagers;
use crate::ui::overlay::OverlayStack;
use std::collections::{BTreeMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_WIDGET_TREE_SCOPE: AtomicU64 = AtomicU64::new(1);

#[path = "tree_layout.rs"]
mod tree_layout;

pub struct WidgetTree {
    tree_scope: u64,
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_slots: Vec<usize>,
    pub(crate) generations: Vec<u32>,
    pub(crate) next_slot: usize,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) scroll_region_move: Option<(Rect, f32, f32)>,
    pub tree_version: u64,
    cached_traversal: std::cell::RefCell<(Vec<WidgetId>, u64)>,

    pub(crate) handler_table: HandlerTable,
    pub(crate) overlay_stack: OverlayStack,

    pub(crate) invalidation: InvalidationQueueHandle,
    pub(crate) reconcile_requested: std::sync::Arc<std::sync::atomic::AtomicBool>,
    pub(crate) effects: Vec<crate::ui::foundation::state::Effect>,
    managers: WidgetManagers,
    app_state: Option<AppState>,
    timer_routes: BTreeMap<u64, (WidgetId, u32)>,
    focus_trap_restore: Vec<(WidgetId, Option<WidgetId>)>,
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self {
            tree_scope: NEXT_WIDGET_TREE_SCOPE.fetch_add(1, Ordering::Relaxed),
            nodes: Vec::new(),
            free_slots: Vec::new(),
            generations: Vec::new(),
            next_slot: 0,
            root_id: None,
            scroll_region_move: None,
            tree_version: 0,
            cached_traversal: std::cell::RefCell::new((Vec::new(), 0)),
            handler_table: HandlerTable::new(),
            overlay_stack: OverlayStack::new(),
            invalidation: crate::draw::pipeline::InvalidationQueue::shared(),
            reconcile_requested: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false)),
            effects: Vec::new(),
            managers: WidgetManagers::new(),
            app_state: None,
            timer_routes: BTreeMap::new(),
            focus_trap_restore: Vec::new(),
        }
    }
}

impl WidgetTree {
    pub(crate) const ROOT_BOOTSTRAP_SIZE: Size = Size { w: 800.0, h: 600.0 };

    pub fn new() -> Self {
        Self::default()
    }

    pub fn tree_version(&self) -> u64 {
        self.tree_version
    }

    pub fn managers(&self) -> &WidgetManagers {
        &self.managers
    }

    pub fn managers_mut(&mut self) -> &mut WidgetManagers {
        &mut self.managers
    }

    pub fn set_app_state(&mut self, app_state: AppState) {
        self.app_state = Some(app_state);
        self.sync_app_state_registry();
    }

    pub fn app_state(&self) -> Option<AppState> {
        self.app_state.clone()
    }

    pub(crate) fn drain_app_state_semantic_events(&mut self) -> bool {
        let Some(app_state) = self.app_state.clone() else {
            return false;
        };
        let targets: HashSet<_> = self.traverse().into_iter().collect();
        let events = app_state.drain_semantic_events_for(&targets);
        if events.is_empty() {
            return false;
        }
        for (id, mut event) in events {
            if self.get(id).is_some() {
                event.target = id;
                event.current_target = id;
                let _ = self.dispatch_semantic(event);
            }
        }
        true
    }

    pub(crate) fn has_app_state_semantic_events(&self) -> bool {
        let Some(app_state) = self.app_state.as_ref() else {
            return false;
        };
        let targets: HashSet<_> = self.traverse().into_iter().collect();
        app_state.has_semantic_events_for(&targets)
    }

    pub(crate) fn register_app_state_snapshot(&self, id: WidgetId) {
        let Some(app_state) = &self.app_state else {
            return;
        };
        let Some(node) = self.get(id) else {
            return;
        };
        let frame = node.frame();
        let dirty = node.dirty_rect(frame);
        let rect = if dirty.w > 0.0 && dirty.h > 0.0 {
            Some(dirty)
        } else if frame.w > 0.0 && frame.h > 0.0 {
            Some(frame)
        } else {
            None
        };
        app_state.register(
            id,
            ComponentConfigSnapshot::from_component(id, node.component()),
            self.invalidation_handle(),
            rect,
        );
    }

    pub(crate) fn unregister_app_state_snapshot(&self, id: WidgetId) {
        if let Some(app_state) = &self.app_state {
            app_state.unregister(id);
        }
    }

    fn sync_app_state_registry(&self) {
        if self.app_state.is_none() {
            return;
        }
        for id in self.traverse() {
            if self.get(id).is_some_and(|node| node.mounted()) {
                self.register_app_state_snapshot(id);
            }
        }
    }

    pub fn alloc_id(&mut self) -> ComponentId {
        if let Some(slot) = self.free_slots.pop() {
            return WidgetId::from_scoped_parts(self.tree_scope, slot, self.generations[slot]);
        }
        let slot = self.next_slot;
        self.next_slot += 1;
        if self.generations.len() <= slot {
            self.generations.push(0);
        }
        WidgetId::from_scoped_parts(self.tree_scope, slot, self.generations[slot])
    }

    fn slot_for(&self, id: WidgetId) -> Option<usize> {
        if id.tree_scope() != self.tree_scope {
            return None;
        }
        let slot = id.slot();
        self.generations
            .get(slot)
            .copied()
            .filter(|&generation| generation == id.generation())?;
        Some(slot)
    }

    fn node_slot_for(&self, id: WidgetId) -> Option<usize> {
        let slot = self.slot_for(id)?;
        self.nodes.get(slot).and_then(|node| node.as_ref())?;
        Some(slot)
    }

    fn invalidate_slot_generation(&mut self, slot: usize) {
        if let Some(generation) = self.generations.get_mut(slot) {
            *generation = generation.wrapping_add(1);
        }
    }

    pub fn set_root_with_children(
        &mut self,
        widget: Box<dyn WidgetComponent>,
        children: Vec<Box<dyn WidgetComponent>>,
    ) -> ComponentId {
        let id = self.set_root(widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    pub fn add_child_with_children(
        &mut self,
        parent_id: ComponentId,
        widget: Box<dyn WidgetComponent>,
        children: Vec<Box<dyn WidgetComponent>>,
    ) -> ComponentId {
        let id = self.add_child(parent_id, widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    fn reset_interaction_state(&mut self) {
        self.managers.focus.clear_tree_focus();
        self.managers.interaction.clear_tree_interaction();
        self.managers.drag.clear_tree_drag();
        self.focus_trap_restore.clear();
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
        self.unregister_app_state_snapshot(id);
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

    pub(crate) fn root_bootstrap_constraints() -> Constraints {
        Constraints::loose(Self::ROOT_BOOTSTRAP_SIZE)
    }

    pub fn set_root(&mut self, widget: Box<dyn WidgetComponent>) -> ComponentId {
        self.teardown_all();

        // Hard reset: clear the old tree and invalidate every previous ComponentId.
        self.nodes.clear();
        self.free_slots.clear();
        for generation in &mut self.generations {
            *generation = generation.wrapping_add(1);
        }
        self.next_slot = 0;
        self.root_id = None;
        self.handler_table.clear();
        self.overlay_stack.clear();
        self.managers.clear_overrides();
        self.reset_interaction_state();
        self.tree_version += 1;

        let children = widget.build();
        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new(widget);
        boxed.set_id(id);
        boxed.set_tab_index(boxed.component().tab_index());
        // Root has no parent content rect yet; window/session layout overwrites this
        // natural fallback once a real viewport is available.
        let ps = boxed.measure(Self::root_bootstrap_constraints());
        boxed.set_frame(Rect::new(0.0, 0.0, ps.w, ps.h));
        let slot = id.slot();
        if self.nodes.len() <= slot {
            self.nodes.resize_with(slot + 1, || None);
        }
        self.nodes[slot] = Some(boxed);
        self.root_id = Some(id);
        self.register_focusable(id);
        self.attach_node(id);
        for child in children {
            self.add_child(id, child);
        }
        self.push_layout_invalidation(id);
        id
    }

    pub fn root(&self) -> Option<&BoxedWidget> {
        self.root_id.and_then(|id| self.get(id))
    }
    pub fn root_id(&self) -> Option<ComponentId> {
        self.root_id
    }
    pub fn root_mut(&mut self) -> Option<&mut BoxedWidget> {
        self.root_id.and_then(|id| self.get_mut(id))
    }

    pub fn find_by_type<T: WidgetComponent + 'static>(&self) -> Option<ComponentId> {
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.component().as_any().downcast_ref::<T>().is_some() {
                    return Some(id);
                }
            }
        }
        None
    }

    pub fn find_all_by_type<T: WidgetComponent + 'static>(&self) -> Vec<(ComponentId, &T)> {
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
    ) -> Option<ComponentId> {
        let id = self.find_by_type::<T>()?;
        if let Some(node) = self.get_mut(id) {
            if let Some(w) = node.component_mut().as_any_mut().downcast_mut::<T>() {
                f(w);
            }
        }
        Some(id)
    }

    pub fn get(&self, id: ComponentId) -> Option<&BoxedWidget> {
        let slot = self.node_slot_for(id)?;
        self.nodes.get(slot).and_then(|n| n.as_ref())
    }
    pub fn get_mut(&mut self, id: ComponentId) -> Option<&mut BoxedWidget> {
        let slot = self.node_slot_for(id)?;
        self.nodes.get_mut(slot).and_then(|n| n.as_mut())
    }

    pub fn set_z_index(&mut self, id: ComponentId, z: i32) -> &mut Self {
        if let Some(n) = self.get_mut(id) {
            n.set_z_index(z);
        }
        self
    }

    pub fn add_child(
        &mut self,
        parent_id: ComponentId,
        child: Box<dyn WidgetComponent>,
    ) -> ComponentId {
        self.tree_version += 1;
        let children = child.build();
        let child_id = self.alloc_id();
        let mut boxed = BoxedWidget::new(child);
        boxed.set_id(child_id);
        boxed.set_parent(Some(parent_id));
        boxed.set_tab_index(boxed.component().tab_index());
        let child_slot = child_id.slot();
        if self.nodes.len() <= child_slot {
            self.nodes.resize_with(child_slot + 1, || None);
        }
        self.nodes[child_slot] = Some(boxed);
        self.register_focusable(child_id);
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

    pub fn remove(&mut self, id: ComponentId) {
        self.tree_version += 1;

        let old_frame = self
            .get(id)
            .map(|n| n.frame())
            .filter(|f| f.w > 0.0 && f.h > 0.0);

        let Some(slot) = self.node_slot_for(id) else {
            return;
        };
        let parent_id = self.nodes[slot].as_ref().and_then(|n| n.parent());
        if self.managers.focus.focused_component() == Some(id) {
            // Dispatch FocusOut while the node is still addressable.
            let _ = self.dispatch_to(id, &SystemEvent::FocusOut);
            self.managers.focus.set_focused_component(None);
        }
        self.teardown_subtree(id);
        if let Some(node) = self.nodes.get_mut(slot) {
            if let Some(node) = node.take() {
                for child_id in node.children().to_vec() {
                    self.remove(child_id);
                }
                self.handler_table.clear_component(id);
                let owner_was_top_trap = self
                    .overlay_stack
                    .top()
                    .is_some_and(|entry| entry.owner() == id && entry.traps_focus());
                let removed_focus_trap = self
                    .overlay_stack
                    .remove_for_owner(id)
                    .iter()
                    .any(|entry| entry.traps_focus());
                if removed_focus_trap {
                    if owner_was_top_trap {
                        self.restore_focus_after_trap_owner(id);
                    } else {
                        let _ = self.take_focus_trap_restore(id);
                    }
                }
                self.managers.remove_overrides(id);
                self.managers.focus.unregister_component(id);
                self.managers.interaction.unregister_component(id);
                self.managers.drag.unregister_component(id);
                self.invalidate_slot_generation(slot);
                self.free_slots.push(slot);
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

    pub fn set_visible(&mut self, id: ComponentId, visible: bool) {
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

    pub fn traverse(&self) -> Vec<ComponentId> {
        let mut cache = self.cached_traversal.borrow_mut();
        let (ref mut ids, ref mut ver) = *cache;
        if *ver != self.tree_version {
            ids.clear();
            if let Some(root_id) = self.root_id {
                // Iterative traversal avoids stack overflow on very deep trees.
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

    pub fn active_timers(&mut self) -> Vec<(u64, std::time::Duration)> {
        self.timer_routes.clear();
        let mut timers = Vec::new();
        for id in self.traverse() {
            if let Some((local_id, delay)) = self.get(id).and_then(|node| node.active_timer()) {
                let Ok(local_timer_id) = u32::try_from(local_id) else {
                    continue;
                };
                let key = Self::timer_work_key(id, local_id);
                self.timer_routes.insert(key, (id, local_timer_id));
                timers.push((key, delay));
            }
        }
        timers
    }

    pub(crate) fn dispatch_timer_work(&mut self, timer_id: u64) -> EventResult {
        if let Some((target, local_id)) = self.timer_routes.get(&timer_id).copied() {
            if self.get(target).is_some() {
                let result = self.dispatch_to(target, &SystemEvent::Timer { id: local_id });
                self.rebuild_widget_overlays();
                return result;
            }
        }

        if let Ok(id) = u32::try_from(timer_id) {
            self.dispatch_event(&SystemEvent::Timer { id })
        } else {
            EventResult::NotHandled
        }
    }

    pub(crate) fn timer_work_key(id: WidgetId, local_id: u64) -> u64 {
        let slot = id.slot() as u64;
        let generation = id.generation() as u64;
        slot.wrapping_mul(0x9E37_79B9_7F4A_7C15)
            ^ generation.rotate_left(17)
            ^ local_id.rotate_left(33)
    }

    pub fn set_frame_dirty(&mut self, id: ComponentId, new_frame: Rect) {
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

    // WidgetNode tree building.

    pub fn build(&mut self, node: WidgetNode) -> ComponentId {
        self.build_node(node, None)
    }

    pub(crate) fn build_child_node(&mut self, parent_id: WidgetId, node: WidgetNode) -> WidgetId {
        self.build_node(node, Some(parent_id))
    }

    pub fn set_children(&mut self, parent_id: ComponentId, children: Vec<WidgetNode>) {
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
        self.register_focusable(id);
        let handler_signatures = handlers
            .iter()
            .map(|handler| handler.authored_signature())
            .collect();
        if let Some(n) = self.get_mut(id) {
            n.set_handler_signatures(handler_signatures);
        }
        for handler in handlers {
            self.handler_table.register(id, handler);
        }
        for child in children {
            self.build_node(child, Some(id));
        }
        id
    }

    pub fn focus_by_type<T: WidgetComponent + 'static>(&mut self) -> Option<ComponentId> {
        let id = self.find_by_type::<T>()?;
        self.managers.focus.set_focused_component(Some(id));
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
        self.managers
            .focus
            .focused_component()
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

    // Tab focus navigation.

    pub fn collect_focusable(&self) -> Vec<ComponentId> {
        let mut result = self
            .managers
            .focus
            .focusable_order()
            .into_iter()
            .filter(|&id| self.get(id).is_some_and(|node| node.is_focusable()))
            .collect::<Vec<_>>();

        for id in self.traverse() {
            if !result.contains(&id) && self.get(id).is_some_and(|node| node.is_focusable()) {
                result.push(id);
            }
        }

        result
    }

    pub(crate) fn is_descendant_of(&self, id: WidgetId, ancestor: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(current_id) = current {
            if current_id == ancestor {
                return true;
            }
            current = self.get(current_id).and_then(|node| node.parent());
        }
        false
    }

    pub(crate) fn collect_focusable_within(&self, root: WidgetId) -> Vec<WidgetId> {
        self.collect_focusable()
            .into_iter()
            .filter(|&id| self.is_descendant_of(id, root))
            .collect()
    }

    fn next_focus_from_order(
        &self,
        focusable: &[WidgetId],
        current: Option<WidgetId>,
        forward: bool,
    ) -> Option<WidgetId> {
        next_focus_in_order(focusable, current, forward)
    }

    pub fn focus_next(&self, forward: bool) -> Option<ComponentId> {
        let focusable = self.collect_focusable();
        self.next_focus_from_order(&focusable, self.managers.focus.focused_component(), forward)
    }

    pub(crate) fn focus_next_in_scope(&self, root: WidgetId, forward: bool) -> Option<WidgetId> {
        let focusable = self.collect_focusable_within(root);
        self.next_focus_from_order(&focusable, self.managers.focus.focused_component(), forward)
    }

    pub(crate) fn remember_focus_before_trap(&mut self, owner: WidgetId) {
        if self
            .focus_trap_restore
            .iter()
            .any(|&(restore_owner, _)| restore_owner == owner)
        {
            return;
        }
        let current = self.managers.focus.focused_component();
        let restore = current.filter(|&id| !self.is_descendant_of(id, owner));
        self.focus_trap_restore.push((owner, restore));
    }

    pub(crate) fn take_focus_trap_restore(&mut self, owner: WidgetId) -> Option<WidgetId> {
        let index = self
            .focus_trap_restore
            .iter()
            .position(|&(restore_owner, _)| restore_owner == owner)?;
        self.focus_trap_restore.remove(index).1
    }

    pub(crate) fn restore_focus_after_trap_owner(&mut self, owner: WidgetId) {
        let restore_focus = self
            .take_focus_trap_restore(owner)
            .filter(|&id| self.get(id).is_some());
        self.set_focus(restore_focus);
    }

    fn register_focusable(&mut self, id: WidgetId) {
        if let Some(node) = self.get(id) {
            self.managers.focus.register_focusable(id, node.tab_index());
        }
    }
}

#[cfg(test)]
#[path = "../../../tests/ui/core/widget/tree_core.rs"]
mod tests;
