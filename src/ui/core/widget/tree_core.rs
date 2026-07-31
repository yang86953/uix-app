use super::*;
use crate::core::{Constraints, Rect, Size};
use crate::draw::renderer::{Invalidation, InvalidationQueueHandle};
use crate::native::windowing::input::{KeyCode, KeyMod};
use crate::ui::animation::AnimatedSource;
use crate::ui::app_state::{AppState, FocusRequest};
use crate::ui::event::{HandlerTable, SemanticEvent, WindowAction};
use crate::ui::focus_handle::FocusHandle;
use crate::ui::foundation::focus_trap::next_focus_in_order;
use crate::ui::managers::WidgetManagers;
use crate::ui::overlay::OverlayStack;
use crate::ui::render_handler::{RenderHandlerRegistration, RenderHandlerTable};
use crate::ui::theme::Theme;
use crate::ui::traits::ThemeTokens;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

static NEXT_WIDGET_TREE_SCOPE: AtomicU64 = AtomicU64::new(1);

struct BoundAnimatedSource {
    tree_scope: u64,
    source: Arc<dyn AnimatedSource>,
}

impl Drop for BoundAnimatedSource {
    fn drop(&mut self) {
        self.source.unbind_owner(self.tree_scope);
    }
}

#[cfg(test)]
thread_local! {
    /// 1=Phase1 2=Phase2 4=Phase4；0=不记录。
    #[allow(
        clippy::missing_const_for_thread_local,
        reason = "the initializer is already const and the lint fires through thread_local"
    )]
    pub(crate) static LAYOUT_TRACE_PHASE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

mod build;
#[path = "tree_layout.rs"]
mod tree_layout;

pub struct WidgetTree {
    tree_scope: u64,
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_slots: Vec<usize>,
    pub(crate) generations: Vec<u32>,
    pub(crate) next_slot: usize,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) scroll_region_moves: Vec<(Rect, f32, f32)>,
    pub(crate) pending_window_actions: Vec<WindowAction>,
    pub tree_version: u64,
    /// UI 域主题令牌根；ScenePaint::paint 用它构造 UI-owned 绘制上下文。
    theme_tokens: std::sync::Arc<dyn ThemeTokens>,
    cached_traversal: std::cell::RefCell<(Vec<WidgetId>, u64)>,

    pub(crate) handler_table: HandlerTable,
    pub(crate) render_handler_table: RenderHandlerTable,
    pub(crate) overlay_stack: OverlayStack,

    pub(crate) invalidation: InvalidationQueueHandle,
    pub(crate) pending_invalidations: Vec<Invalidation>,
    pub(crate) invalidation_batch_depth: usize,
    pub(crate) layout_ancestor_scratch: Vec<WidgetId>,
    pub(crate) reconcile_requested: Arc<AtomicBool>,
    pub(crate) reconcile_callback: Arc<dyn Fn() + Send + Sync>,
    pub(crate) effects: Vec<crate::ui::foundation::state::Effect>,
    animated_sources: BTreeMap<WidgetId, BoundAnimatedSource>,
    active_component_animations: HashSet<WidgetId>,
    animation_ids_scratch: Vec<WidgetId>,
    lifecycle_states_scratch: Vec<(WidgetId, bool)>,
    layout_scratch: tree_layout::LayoutFrameScratch,
    app_state_semantic_events_scratch: Vec<(WidgetId, SemanticEvent)>,
    app_state_focus_requests_scratch: Vec<(WidgetId, FocusRequest)>,
    managers: WidgetManagers,
    app_state: Option<AppState>,
    focus_handles: HashMap<WidgetId, FocusHandle>,
    timer_routes: BTreeMap<u64, (WidgetId, u32)>,
    focus_trap_restore: Vec<(WidgetId, Option<WidgetId>)>,
    pub(crate) window_focused: bool,
    keyboard_focus_visible: bool,
    pub(crate) keyboard_activation: Option<(WidgetId, KeyCode, KeyMod)>,
    #[cfg(feature = "test-harness")]
    pub(crate) automation_recorder: Option<crate::ui::automation::AutomationRecorder>,
    /// layout() 内实际改写 frame 次数（回归：收敛后二次 layout 应为 0）。
    #[cfg(test)]
    pub(crate) layout_frame_writes: std::cell::Cell<u32>,
    /// Phase 4 实际执行的 shrink 次数（回归：Stretch 侧栏不应反复 shrink）。
    #[cfg(test)]
    pub(crate) layout_shrink_ops: std::cell::Cell<u32>,
    /// 单次 layout() 收敛循环实际执行的遍数（含最后稳定遍）。
    #[cfg(test)]
    pub(crate) layout_converge_passes: std::cell::Cell<u32>,
    /// Phase 2 实际扩展 frame 的次数（回归：不得在稳定后反复 120→124）。
    #[cfg(test)]
    pub(crate) layout_expand_ops: std::cell::Cell<u32>,
    /// 测试探针：记录 `(phase, id, before_h, after_h)` 的 frame 写入。
    #[cfg(test)]
    pub(crate) layout_frame_trace: std::cell::RefCell<Vec<(u8, ComponentId, i32, i32)>>,
}

impl Default for WidgetTree {
    fn default() -> Self {
        let reconcile_requested = Arc::new(AtomicBool::new(false));
        let reconcile_callback = {
            let requested = Arc::clone(&reconcile_requested);
            Arc::new(move || requested.store(true, Ordering::Release))
                as Arc<dyn Fn() + Send + Sync>
        };
        Self {
            tree_scope: NEXT_WIDGET_TREE_SCOPE.fetch_add(1, Ordering::Relaxed),
            nodes: Vec::new(),
            free_slots: Vec::new(),
            generations: Vec::new(),
            next_slot: 0,
            root_id: None,
            theme_tokens: Theme::antd_light().tokens_arc(),
            scroll_region_moves: Vec::new(),
            pending_window_actions: Vec::new(),
            tree_version: 0,
            cached_traversal: std::cell::RefCell::new((Vec::new(), 0)),
            handler_table: HandlerTable::new(),
            render_handler_table: RenderHandlerTable::default(),
            overlay_stack: OverlayStack::new(),
            invalidation: crate::draw::renderer::InvalidationQueue::shared(),
            pending_invalidations: Vec::new(),
            invalidation_batch_depth: 0,
            layout_ancestor_scratch: Vec::new(),
            reconcile_requested,
            reconcile_callback,
            effects: Vec::new(),
            animated_sources: BTreeMap::new(),
            active_component_animations: HashSet::new(),
            animation_ids_scratch: Vec::new(),
            lifecycle_states_scratch: Vec::new(),
            layout_scratch: tree_layout::LayoutFrameScratch::default(),
            app_state_semantic_events_scratch: Vec::new(),
            app_state_focus_requests_scratch: Vec::new(),
            managers: WidgetManagers::new(),
            app_state: None,
            focus_handles: HashMap::new(),
            timer_routes: BTreeMap::new(),
            focus_trap_restore: Vec::new(),
            window_focused: true,
            keyboard_focus_visible: true,
            keyboard_activation: None,
            #[cfg(feature = "test-harness")]
            automation_recorder: None,
            #[cfg(test)]
            layout_frame_writes: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_shrink_ops: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_converge_passes: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_expand_ops: std::cell::Cell::new(0),
            #[cfg(test)]
            layout_frame_trace: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl WidgetTree {
    pub(crate) const ROOT_BOOTSTRAP_SIZE: Size = Size { w: 800.0, h: 600.0 };

    pub(crate) fn keyboard_focus_visible(&self) -> bool {
        self.window_focused && self.keyboard_focus_visible
    }

    pub(crate) fn set_keyboard_focus_visible(&mut self, visible: bool) {
        if self.keyboard_focus_visible == visible {
            return;
        }
        self.keyboard_focus_visible = visible;
        if let Some(focused) = self.managers.focus.focused_component() {
            self.invalidate_paint(focused);
        }
    }

    pub(crate) fn set_theme_tokens(&mut self, tokens: std::sync::Arc<dyn ThemeTokens>) {
        self.theme_tokens = tokens;
    }

    /// 当前 UI 域主题令牌根。
    pub(crate) fn theme_tokens(&self) -> std::sync::Arc<dyn ThemeTokens> {
        self.theme_tokens.clone()
    }

    pub fn new() -> Self {
        Self::default()
    }

    pub fn tree_version(&self) -> u64 {
        self.tree_version
    }

    pub(crate) fn sync_animated_sources(&mut self, sources: Vec<Arc<dyn AnimatedSource>>) {
        let mut previous = std::mem::take(&mut self.animated_sources);
        let mut next = BTreeMap::new();
        for source in sources {
            let work_id = source.work_id();
            if next.contains_key(&work_id) {
                continue;
            }
            if let Some(bound) = previous.remove(&work_id) {
                next.insert(work_id, bound);
            } else if source.bind_owner(self.tree_scope) {
                next.insert(
                    work_id,
                    BoundAnimatedSource {
                        tree_scope: self.tree_scope,
                        source,
                    },
                );
            }
        }
        self.animated_sources = next;
    }

    pub(crate) fn take_window_actions(&mut self) -> Vec<WindowAction> {
        std::mem::take(&mut self.pending_window_actions)
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
        let mut events = std::mem::take(&mut self.app_state_semantic_events_scratch);
        app_state.drain_semantic_events_for_scope_into(self.tree_scope, &mut events);
        if events.is_empty() {
            self.app_state_semantic_events_scratch = events;
            return false;
        }
        for (id, mut event) in events.drain(..) {
            if self.get(id).is_some() {
                event.target = id;
                event.current_target = id;
                let _ = self.dispatch_semantic(event);
            }
        }
        self.app_state_semantic_events_scratch = events;
        true
    }

    pub(crate) fn has_app_state_semantic_events(&self) -> bool {
        let Some(app_state) = self.app_state.as_ref() else {
            return false;
        };
        app_state.has_semantic_events_for_scope(self.tree_scope)
    }

    pub(crate) fn drain_app_state_focus_requests(&mut self) -> bool {
        let Some(app_state) = self.app_state.clone() else {
            return false;
        };
        let mut requests = std::mem::take(&mut self.app_state_focus_requests_scratch);
        app_state.drain_focus_requests_for_scope_into(self.tree_scope, &mut requests);
        if requests.is_empty() {
            self.app_state_focus_requests_scratch = requests;
            return false;
        }
        for (id, request) in requests.drain(..) {
            match request {
                crate::ui::app_state::FocusRequest::Focus => {
                    if self.focus_target_available(id) {
                        self.set_keyboard_focus_visible(true);
                        self.set_focus(Some(id));
                    }
                }
                crate::ui::app_state::FocusRequest::FocusAndReveal => {
                    if self.focus_target_available(id) {
                        self.set_keyboard_focus_visible(true);
                        self.set_focus(Some(id));
                        self.reveal_focused_target(id);
                    }
                }
                crate::ui::app_state::FocusRequest::Blur => {
                    if self.managers.focus.focused_component() == Some(id) {
                        self.set_focus(None);
                    }
                }
            }
        }
        self.app_state_focus_requests_scratch = requests;
        true
    }

    pub(crate) fn has_app_state_focus_requests(&self) -> bool {
        let Some(app_state) = self.app_state.as_ref() else {
            return false;
        };
        app_state.has_focus_requests_for_scope(self.tree_scope)
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
        }
        .and_then(|rect| self.node_visual_rect(id, rect));
        app_state.register(
            id,
            node.component_snapshot(id),
            self.invalidation_handle(),
            rect,
        );
        if let Some(handle) = self.focus_handles.get(&id) {
            handle.bind(id, app_state);
        }
    }

    pub(crate) fn unregister_app_state_snapshot(&self, id: WidgetId) {
        if let Some(handle) = self.focus_handles.get(&id) {
            handle.unbind(id);
        }
        if let Some(app_state) = &self.app_state {
            app_state.unregister(id);
        }
    }

    fn sync_app_state_registry(&self) {
        if self.app_state.is_none() {
            return;
        }
        for &id in self.traverse().iter() {
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
        self.keyboard_activation = None;
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
        let ids: Vec<_> = self.traverse().iter().copied().collect();
        for id in ids.into_iter().rev() {
            self.deactivate_detach_and_destroy(id);
        }
    }

    /// Ends the tree's mounted lifecycle before its owning window releases
    /// rendering resources. Repeated shutdown is harmless because each node's
    /// lifecycle flags suppress duplicate callbacks.
    pub(crate) fn shutdown(&mut self) {
        self.teardown_all();
    }

    pub fn notify_theme_changed(&mut self) {
        let ids: Vec<_> = self.traverse().iter().copied().collect();
        for id in ids {
            if let Some(node) = self.get_mut(id) {
                node.on_theme_changed();
            }
            if self.get(id).is_some_and(|node| node.uses_palette()) {
                self.invalidate_paint(id);
            }
        }
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
        for &id in self.traverse().iter() {
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
        for &id in self.traverse().iter() {
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

    pub fn remove(&mut self, id: ComponentId) {
        self.tree_version += 1;
        self.active_component_animations.remove(&id);

        let old_visual_bounds = self.visual_subtree_bounds(id);

        let Some(slot) = self.node_slot_for(id) else {
            return;
        };
        let parent_id = self.nodes[slot].as_ref().and_then(|n| n.parent());
        // 节点仍可寻址时交付 PointerLeave / DragEnd / FocusOut，再销毁生命周期。
        self.cancel_subtree_interaction(id);
        self.teardown_subtree(id);
        if let Some(node) = self.nodes.get_mut(slot) {
            if let Some(node) = node.take() {
                for child_id in node.children().to_vec() {
                    self.remove(child_id);
                }
                self.handler_table.clear_component(id);
                self.render_handler_table.clear_component(id);
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
                self.focus_handles.remove(&id);
                self.invalidate_slot_generation(slot);
                self.free_slots.push(slot);
            }
        }
        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_mut(pid) {
                parent.children_mut().retain(|&c| c != id);
            }
        }

        if let Some(rect) = old_visual_bounds {
            if let Some(pid) = parent_id {
                self.push_paint_invalidation(pid, Some(rect));
            }
        }
        if let Some(pid) = parent_id {
            self.push_layout_invalidation(pid);
            self.propagate_layout_invalidation(pid);
        }
    }

    pub fn set_visible(&mut self, id: ComponentId, visible: bool) {
        if !visible {
            self.cancel_subtree_interaction(id);
        }

        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let children: Vec<WidgetId> = self
                .get(current)
                .map(|n| n.children().to_vec())
                .unwrap_or_default();

            let mut changed = false;
            if let Some(n) = self.get_mut(current) {
                if n.visibility_gate() != visible {
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

    /// 返回树的先序遍历缓存；借用守卫存活期间不得修改树结构。
    pub fn traverse(&self) -> std::cell::Ref<'_, [ComponentId]> {
        {
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
        }
        std::cell::Ref::map(self.cached_traversal.borrow(), |(ids, _)| ids.as_slice())
    }

    pub fn active_timers(&mut self) -> Vec<(u64, std::time::Duration)> {
        let mut timer_routes = std::mem::take(&mut self.timer_routes);
        timer_routes.clear();
        let mut timers = Vec::new();
        for &id in self.traverse().iter() {
            if self.is_pending_removal_subtree(id) {
                continue;
            }
            if let Some((local_id, delay)) = self.get(id).and_then(|node| node.active_timer()) {
                let Ok(local_timer_id) = u32::try_from(local_id) else {
                    continue;
                };
                let key = Self::timer_work_key(id, local_id);
                timer_routes.insert(key, (id, local_timer_id));
                timers.push((key, delay));
            }
        }
        self.timer_routes = timer_routes;
        timers
    }

    pub(crate) fn dispatch_timer_work(&mut self, timer_id: u64) -> EventResult {
        self.begin_invalidation_batch();
        let result = self.dispatch_timer_work_inner(timer_id);
        self.finish_invalidation_batch();
        result
    }

    fn dispatch_timer_work_inner(&mut self, timer_id: u64) -> EventResult {
        if let Some((target, local_id)) = self.timer_routes.get(&timer_id).copied() {
            if self.get(target).is_some() && !self.is_pending_removal_subtree(target) {
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
        if !self.apply_frame_paint(id, new_frame) {
            return;
        }
        self.push_layout_invalidation(id);
        self.propagate_layout_invalidation(id);
    }

    /// layout() 内写 frame：只标 Paint，不重新入队 Layout。
    ///
    /// `set_frame_dirty` 会 `push_layout_invalidation`，若在收敛循环里调用，
    /// 会在结果已稳定后仍留下 Layout pending，下一帧无事件也再跑 layout（违反休眠）。
    pub(crate) fn set_layout_frame(&mut self, id: ComponentId, new_frame: Rect) -> bool {
        #[cfg(test)]
        let before_h = self
            .get(id)
            .map(|n| n.frame().h.round() as i32)
            .unwrap_or(0);
        if !self.apply_frame_paint(id, new_frame) {
            return false;
        }
        #[cfg(test)]
        {
            self.layout_frame_writes
                .set(self.layout_frame_writes.get().wrapping_add(1));
            let phase = LAYOUT_TRACE_PHASE.with(|p| p.get());
            if phase != 0 {
                self.layout_frame_trace.borrow_mut().push((
                    phase,
                    id,
                    before_h,
                    new_frame.h.round() as i32,
                ));
            }
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn take_layout_frame_trace(&self) -> Vec<(u8, ComponentId, i32, i32)> {
        std::mem::take(&mut *self.layout_frame_trace.borrow_mut())
    }

    fn apply_frame_paint(&mut self, id: ComponentId, new_frame: Rect) -> bool {
        match self.get(id) {
            Some(w) => {
                let old = w.frame();
                if old == new_frame {
                    return false;
                }
            }
            None => return false,
        }
        let old_visual_bounds = self.visual_subtree_bounds(id);
        if let Some(w) = self.get_mut(id) {
            w.set_frame(new_frame);
        }
        let new_visual_bounds = self.visual_subtree_bounds(id);
        for rect in [old_visual_bounds, new_visual_bounds].into_iter().flatten() {
            if rect.w > 0.0 && rect.h > 0.0 {
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        true
    }

    #[cfg(test)]
    pub(crate) fn take_layout_frame_writes(&self) -> u32 {
        self.layout_frame_writes.replace(0)
    }

    #[cfg(test)]
    pub(crate) fn take_layout_shrink_ops(&self) -> u32 {
        self.layout_shrink_ops.replace(0)
    }

    #[cfg(test)]
    pub(crate) fn take_layout_converge_passes(&self) -> u32 {
        self.layout_converge_passes.replace(0)
    }

    #[cfg(test)]
    pub(crate) fn take_layout_expand_ops(&self) -> u32 {
        self.layout_expand_ops.replace(0)
    }

    pub fn focus_by_type<T: WidgetComponent + 'static>(&mut self) -> Option<ComponentId> {
        let id = self.find_by_type::<T>()?;
        self.set_focus(Some(id));
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

    pub(crate) fn replace_render_handlers(
        &mut self,
        id: ComponentId,
        handlers: Vec<RenderHandlerRegistration>,
    ) {
        self.render_handler_table.replace_component(id, handlers);
    }

    pub(crate) fn replace_system_event_handlers(
        &mut self,
        id: ComponentId,
        handlers: Vec<crate::ui::system_event_handler::SystemEventHandlerRegistration>,
    ) {
        if let Some(node) = self.get_mut(id) {
            node.replace_system_event_handlers(handlers);
        }
    }

    pub(crate) fn set_tab_index_override(&mut self, id: ComponentId, tab_index: Option<i32>) {
        if let Some(node) = self.get_mut(id) {
            node.set_tab_index_override(tab_index);
        }
        self.register_focusable(id);
    }

    pub(crate) fn set_focus_handle(&mut self, id: ComponentId, handle: Option<FocusHandle>) {
        let unchanged = self
            .focus_handles
            .get(&id)
            .zip(handle.as_ref())
            .is_some_and(|(current, next)| current.same_handle(next));
        if unchanged {
            return;
        }
        if let Some(previous) = self.focus_handles.remove(&id) {
            previous.unbind(id);
        }
        let Some(handle) = handle else {
            return;
        };
        if let Some(app_state) = &self.app_state {
            handle.bind(id, app_state);
        }
        self.focus_handles.insert(id, handle);
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
            .filter(|&id| self.is_tab_focus_candidate(id))
            .collect::<Vec<_>>();

        for &id in self.traverse().iter() {
            if !result.contains(&id) && self.is_tab_focus_candidate(id) {
                result.push(id);
            }
        }

        result
    }

    pub(crate) fn is_effectively_visible(&self, id: WidgetId) -> bool {
        let mut current = Some(id);
        while let Some(current_id) = current {
            let Some(node) = self.get(current_id) else {
                return false;
            };
            if !node.visible() {
                return false;
            }
            current = node.parent();
        }
        true
    }

    fn is_tab_focus_candidate(&self, id: WidgetId) -> bool {
        self.is_effectively_visible(id)
            && !self.is_pending_removal_subtree(id)
            && self.get(id).is_some_and(|node| node.is_focusable())
    }

    pub(crate) fn focus_target_available(&self, id: WidgetId) -> bool {
        self.is_effectively_visible(id)
            && !self.is_pending_removal_subtree(id)
            && self
                .get(id)
                .is_some_and(|node| node.accepts_events() && node.is_interaction_enabled())
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
            .filter(|&id| self.focus_target_available(id));
        self.set_focus(restore_focus);
    }

    fn register_focusable(&mut self, id: WidgetId) {
        if let Some(node) = self.get(id) {
            self.managers.focus.register_focusable(id, node.tab_index());
        }
    }
}
