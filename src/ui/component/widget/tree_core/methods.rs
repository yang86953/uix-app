use super::*;
use crate::core::{Rect, Size};
use crate::ui::component::app_state::AppState;
use crate::ui::component::managers::WidgetManagers;
use crate::ui::event::WindowAction;
use crate::ui::theme::traits::ThemeTokens;

impl WidgetTree {
    // 使用已捕获根绑定的存储创建窗口树，避免首次构建与后续协调分裂状态所有权。
    pub(crate) fn with_component_state_store(
        // 接收首次根捕获已经使用的窗口私有存储。
        store: crate::ui::component_state::ComponentStateStore,
    ) -> Self {
        // 先建立其余默认运行时资源。
        let mut tree = Self::default();
        // 再用捕获根指定的窗口私有存储替换默认值。
        tree.component_state_store = store;
        // 返回拥有单一状态存储的树。
        tree
    }

    // 返回当前树唯一拥有的组件私有状态存储句柄。
    pub(crate) fn component_state_store(&self) -> crate::ui::component_state::ComponentStateStore {
        // 克隆轻量共享句柄而不复制任何状态。
        self.component_state_store.clone()
    }

    // 开始适配器的构建或协调事务并推迟作用域状态清理。
    fn begin_component_state_transaction(&mut self) {
        // 允许嵌套构建入口而不提前释放仍会在本轮重挂载的状态。
        self.component_state_transaction_depth = self
            // 使用饱和递增防止异常嵌套下整数回绕。
            .component_state_transaction_depth
            // 增加当前树的协调事务深度。
            .saturating_add(1);
    }

    // 结束适配器事务；最终作用域解析由最外层成功入口统一执行。
    fn end_component_state_transaction(&mut self) {
        // 防御性忽略不成对结束，避免测试辅助路径下溢。
        if self.component_state_transaction_depth == 0 {
            // 没有事务时无需再次清理。
            return;
        }
        // 释放一层事务深度。
        self.component_state_transaction_depth -= 1;
        // 最外层成功入口会把最终挂载作用域与本批回执一起原子解析。
    }

    // 在异常展开路径恢复一层事务深度，保留既有挂载状态且不执行破坏性清理。
    fn abort_component_state_transaction(&mut self) {
        // 没有活跃事务时保持调用幂等。
        if self.component_state_transaction_depth == 0 {
            // 提前返回避免深度下溢。
            return;
        }
        // 仅撤销当前入口增加的一层深度。
        self.component_state_transaction_depth -= 1;
    }

    // 在异常安全边界内执行一次组件状态建树或协调事务。
    pub(crate) fn with_component_state_transaction<R>(
        &mut self,
        // 接收由本事务及其声明子树创建的待确认状态 journal。
        receipts: Vec<crate::ui::component_state::ComponentStateCaptureReceipt>,
        // 接收事务期间唯一可变访问当前树的同步闭包。
        action: impl FnOnce(&mut Self) -> R,
    ) -> R {
        // 只保留由当前 WidgetTree 唯一状态存储产生的回执。
        let receipts = receipts
            // 消费输入集合，让错误 store 回执在过滤时立即 Drop 回滚。
            .into_iter()
            // 禁止其他树或一次性捕获的状态 journal 被当前树接纳。
            .filter(|receipt| receipt.belongs_to(&self.component_state_store))
            // 收集可安全加入本树最外层事务的回执。
            .collect::<Vec<_>>();
        // 记录进入本层前的回执边界，供 panic 精确回滚本层及成功嵌套层。
        let checkpoint = self.component_state_pending_receipts.len();
        // 记录进入本层前的动画源替换边界，供 panic 保留既有树绑定。
        let animation_checkpoint = self.pending_animated_source_owner_updates.len();
        // 开启一层延迟清理事务。
        self.begin_component_state_transaction();
        // 将本层声明捕获产生的 journal 交给当前树暂存。
        self.component_state_pending_receipts.extend(receipts);
        // 捕获 panic 以确保事务深度在所有退出路径恢复。
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| action(self)));
        // 区分正常提交与异常回滚。
        match result {
            // 正常路径按最终挂载树清理缺席作用域。
            Ok(value) => {
                // 提交当前事务层。
                self.end_component_state_transaction();
                // 仅最外层成功并完成作用域清理后才统一接纳所有 journal。
                if self.component_state_transaction_depth == 0 {
                    // 在最外层成功后才让本轮动画源所有权替换进入真实树注册表。
                    self.commit_pending_animated_source_owner_updates();
                    // 取走全部已成功挂载的回执，避免后续 Drop 回滚。
                    let receipts = std::mem::take(&mut self.component_state_pending_receipts);
                    // 收集事务成功后仍由实际节点承载的最终作用域真相。
                    let live_scopes = self.component_state_live_scopes();
                    // 原子提交 live claim、释放缺席 claim 并保留其他外部未决捕获。
                    crate::ui::component_state::resolve_component_state_receipts(
                        // 传入当前窗口树唯一的状态存储。
                        &self.component_state_store,
                        // 传入最外层事务积累的完整回执批次。
                        receipts,
                        // 传入最终运行时树承载的作用域集合。
                        &live_scopes,
                    );
                }
                // 返回调用方结果。
                value
            }
            // 异常路径仅恢复深度并继续原始展开。
            Err(payload) => {
                // 取走本层开始后加入的回执，包含所有成功的嵌套事务回执。
                let receipts = self.component_state_pending_receipts.split_off(checkpoint);
                // 立即丢弃本层回执以精确撤销对应的新建状态槽。
                drop(receipts);
                // 丢弃本层及其成功嵌套层的动画源请求，旧树注册保持不变。
                let updates = self
                    .pending_animated_source_owner_updates
                    .split_off(animation_checkpoint);
                // 释放未提交来源，确保异常路径不会产生树作用域绑定。
                drop(updates);
                // 避免基于半完成树执行破坏性 prune。
                self.abort_component_state_transaction();
                // 保持调用方观察到原始 panic。
                std::panic::resume_unwind(payload)
            }
        }
    }

    // 在事务外实际移除节点后立即释放不再被任何节点承载的作用域状态。
    pub(crate) fn prune_component_state_scopes_if_idle(&mut self) {
        // leave 过渡或协调事务期间必须保留状态直到最终树稳定。
        if self.component_state_transaction_depth == 0 {
            // 删除真正缺席的条件分支状态。
            self.prune_component_state_scopes();
        }
    }

    // 收集实际仍挂载的作用域并交给窗口私有存储执行清理。
    fn prune_component_state_scopes(&mut self) {
        // 收集当前实际运行时树承载的全部作用域。
        let live_scopes = self.component_state_live_scopes();
        // 仅删除本树中已没有承载节点的私有状态。
        self.component_state_store.retain_scopes(&live_scopes);
    }

    // 收集当前实际运行时树仍承载的全部组件私有状态作用域。
    fn component_state_live_scopes(&self) -> std::collections::HashSet<UixComponentScope> {
        // 建立不受节点遍历借用影响的作用域集合。
        let mut live_scopes = std::collections::HashSet::<UixComponentScope>::new();
        // 遍历所有尚未从树中实际删除的节点。
        for &id in self.traverse().iter() {
            // 读取当前节点保留的全部嵌套组件标记。
            if let Some(node) = self.get(id) {
                // 一个实际节点可能同时承载多层内联组件作用域。
                for marker in node.uix_component_scopes() {
                    // 记录作用域身份而忽略仅用于节点身份的根序号。
                    live_scopes.insert(marker.scope().clone());
                }
            }
        }
        // 返回供事务解析或事务外 prune 使用的最终集合。
        live_scopes
    }

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
                crate::ui::component::app_state::FocusRequest::Focus => {
                    if self.focus_target_available(id) {
                        self.set_keyboard_focus_visible(true);
                        self.set_focus(Some(id));
                    }
                }
                crate::ui::component::app_state::FocusRequest::FocusAndReveal => {
                    if self.focus_target_available(id) {
                        self.set_keyboard_focus_visible(true);
                        self.set_focus(Some(id));
                        self.reveal_focused_target(id);
                    }
                }
                crate::ui::component::app_state::FocusRequest::Blur => {
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

    pub(crate) fn sync_app_state_registry(&self) {
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

    pub(crate) fn slot_for(&self, id: WidgetId) -> Option<usize> {
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

    pub(crate) fn node_slot_for(&self, id: WidgetId) -> Option<usize> {
        let slot = self.slot_for(id)?;
        self.nodes.get(slot).and_then(|node| node.as_ref())?;
        Some(slot)
    }

    pub(crate) fn invalidate_slot_generation(&mut self, slot: usize) {
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

    pub(crate) fn reset_interaction_state(&mut self) {
        self.managers.focus.clear_tree_focus();
        self.managers.interaction.clear_tree_interaction();
        self.managers.drag.clear_tree_drag();
        self.keyboard_activation = None;
        self.focus_trap_restore.clear();
    }

    pub(crate) fn collect_lifecycle_subtree(&self, id: WidgetId, out: &mut Vec<WidgetId>) {
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

    pub(crate) fn attach_node(&mut self, id: WidgetId) {
        if let Some(node) = self.get_mut(id) {
            if !node.attached() {
                node.set_attached(true);
                node.on_attach();
            }
        }
    }

    pub(crate) fn deactivate_detach_and_destroy(&mut self, id: WidgetId) {
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

    pub(crate) fn teardown_all(&mut self) {
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
        // 释放根生命周期持有的结构性 State 订阅租约。
        self.root_reconcile_state_binds.clear();
        // 释放根捕获的 Effect，避免关闭后继续保留待处理工作。
        self.effects.clear();
        // 清空全部所有者分区，确保关闭不再保留根或节点声明。
        self.animated_source_owners.clear();
        // 取消关闭前尚未提交的全部动画源替换，同时保持活跃事务检查点有效。
        self.cancel_pending_animated_source_owner_updates(|_| true);
        // 释放全部捕获的动画源，并由绑定包装器解除源端所有权。
        self.animated_sources.clear();
        // 关闭后不再保留任何组件动画活动标记。
        self.active_component_animations.clear();
        // 清空动画遍历暂存，避免关闭后的旧节点身份继续存活。
        self.animation_ids_scratch.clear();
        // 释放仍由未移除节点持有的 State 租约与 Effect。
        for node in self.nodes.iter_mut().flatten() {
            // 关闭节点结构性 State 订阅。
            node.clear_reconcile_state_binds();
            // 清空节点 Effect，关闭后不再参与调度。
            node.replace_captured_effects(Vec::new());
        }
        // 释放所有延迟 View 工厂及其应用捕获资源，关闭后不得保留 sidecar。
        self.render_handler_table.clear();
        // 窗口关闭后不再允许任何组件私有状态继续存活。
        self.component_state_store.clear();
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
                // 节点已真实离开槽位后，取消其待提交项并立即释放实际动画源所有权。
                self.release_node_animated_sources_immediately(id);
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
                // 从父节点的直接子节点集合中移除已销毁标识。
                parent.children_mut().retain(|&c| c != id);
                // 让父组件立即清理依赖旧子节点集合的派生运行态。
                parent.notify_children_changed();
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
        // 事务外的实际移除（含 leave 结束）现在可释放无承载节点的私有状态。
        self.prune_component_state_scopes_if_idle();
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

    pub(crate) fn dispatch_timer_work_inner(&mut self, timer_id: u64) -> EventResult {
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

    // 测试目标保留布局帧 trace 取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_frame_trace(&self) -> Vec<(u8, ComponentId, i32, i32)> {
        std::mem::take(&mut *self.layout_frame_trace.borrow_mut())
    }

    pub(crate) fn apply_frame_paint(&mut self, id: ComponentId, new_frame: Rect) -> bool {
        // 比较与脏区计算前先归一，避免等价非法输入制造重复失效。
        let new_frame = crate::ui::layout::engine::normalize_layout_rect(new_frame);
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

    // 测试目标保留布局写入计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_frame_writes(&self) -> u32 {
        self.layout_frame_writes.replace(0)
    }

    // 测试目标保留 shrink 操作计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_shrink_ops(&self) -> u32 {
        self.layout_shrink_ops.replace(0)
    }

    // 测试目标保留收敛 pass 计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_converge_passes(&self) -> u32 {
        self.layout_converge_passes.replace(0)
    }

    // 测试目标保留 expand 操作计数取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_expand_ops(&self) -> u32 {
        self.layout_expand_ops.replace(0)
    }
}
