use super::*;
// 提供帧与脏区操作使用的矩形类型。
use crate::core::Rect;
use crate::ui::event::WindowAction;
use crate::ui::theme::traits::ThemeTokens;
use crate::ui::widget_runtime::app_state::AppState;
use crate::ui::widget_runtime::managers::WidgetManagers;

impl WidgetTree {
    // 在事务外实际移除节点后立即释放不再被任何节点承载的作用域状态。
    pub(crate) fn prune_widget_state_scopes_if_idle(&mut self) {
        // leave 过渡或协调事务期间必须保留状态直到最终树稳定。
        if self.widget_state_transaction_depth == 0 {
            // 删除真正缺席的条件分支状态。
            self.prune_widget_state_scopes();
        }
    }

    // 收集实际仍挂载的作用域并交给窗口私有存储执行清理。
    fn prune_widget_state_scopes(&mut self) {
        // 收集当前实际运行时树承载的全部作用域。
        let live_scopes = self.widget_state_live_scopes();
        // 仅删除本树中已没有承载节点的私有状态。
        self.widget_state_store.retain_scopes(&live_scopes);
    }

    // 收集当前实际运行时树仍承载的全部组件私有状态作用域。
    pub(super) fn widget_state_live_scopes(&self) -> std::collections::HashSet<UixWidgetScope> {
        // 建立不受节点遍历借用影响的作用域集合。
        let mut live_scopes = std::collections::HashSet::<UixWidgetScope>::new();
        // 遍历所有尚未从树中实际删除的节点。
        for &id in self.traverse().iter() {
            // 读取当前节点保留的全部嵌套组件标记。
            if let Some(node) = self.get(id) {
                // 一个实际节点可能同时承载多层内联组件作用域。
                for marker in node.uix_widget_scopes() {
                    // 记录作用域身份而忽略仅用于节点身份的根序号。
                    live_scopes.insert(marker.scope().clone());
                }
            }
        }
        // 返回供事务解析或事务外 prune 使用的最终集合。
        live_scopes
    }

    pub(crate) fn keyboard_focus_visible(&self) -> bool {
        self.window_focused && self.keyboard_focus_visible
    }

    pub(crate) fn set_keyboard_focus_visible(&mut self, visible: bool) {
        if self.keyboard_focus_visible == visible {
            return;
        }
        self.keyboard_focus_visible = visible;
        if let Some(focused) = self.managers.focus.focused_widget() {
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

    /// 创建拥有独立树作用域、管理器和失效状态的空组件树。
    pub fn new() -> Self {
        Self::default()
    }

    /// 返回节点结构每次发布变化后递增的树版本。
    pub fn tree_version(&self) -> u64 {
        self.tree_version
    }

    pub(crate) fn take_window_actions(&mut self) -> Vec<WindowAction> {
        // 非运行态树不得向平台提交协调中或失败前遗留的窗口动作。
        if !self.accepts_external_work() {
            // 永久停止后直接丢弃动作，协调中则保留到成功线性化之后。
            if self.is_fail_stopped() {
                // 清除无法再安全归因到完整树的旧动作。
                self.pending_window_actions.clear();
            }
            // 当前入口不向平台暴露任何半提交事实。
            return Vec::new();
        }
        std::mem::take(&mut self.pending_window_actions)
    }

    /// 返回焦点、交互、拖拽等树级管理器的共享借用。
    pub fn managers(&self) -> &WidgetManagers {
        &self.managers
    }

    /// 返回树级管理器的可变借用；停止树不允许重新建立运行态。
    pub fn managers_mut(&mut self) -> &mut WidgetManagers {
        // 停止树不得重新建立任何交互、焦点或拖拽运行态。
        assert!(self.accepts_coordination_work());
        &mut self.managers
    }

    /// 接管 AppState 句柄并事务化同步当前已挂载节点快照。
    pub fn set_app_state(&mut self, app_state: AppState) {
        // AppState 快照会调用组件快照能力，公开入口必须建立异常事务边界。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 外部注册一旦开始便不可回滚，先记录真实资源发布。
            tree.mark_coordination_publish_started();
            // 接管新的应用状态共享句柄。
            tree.app_state = Some(app_state);
            // 把当前挂载节点同步到新注册表，panic 后由事务 fail-stop。
            tree.sync_app_state_registry();
        });
    }

    /// 返回当前 AppState 共享句柄；尚未配置时返回空值。
    pub fn app_state(&self) -> Option<AppState> {
        self.app_state.clone()
    }

    pub(crate) fn drain_app_state_semantic_events(&mut self) -> bool {
        // 非运行态树不得消费队列或触发语义 handler。
        if !self.accepts_external_work() {
            // 保留 AppState 事实供所属窗口 teardown 丢弃，当前帧没有安全工作。
            return false;
        }
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
        // 非运行态树不向窗口循环暴露无法安全执行的语义工作。
        if !self.accepts_external_work() {
            // 防止 fail-stop owner 因旧 AppState 队列持续保持 Active。
            return false;
        }
        let Some(app_state) = self.app_state.as_ref() else {
            return false;
        };
        app_state.has_semantic_events_for_scope(self.tree_scope)
    }

    pub(crate) fn drain_app_state_focus_requests(&mut self) -> bool {
        // 非运行态树不得消费焦点请求或执行节点焦点生命周期。
        if !self.accepts_external_work() {
            // 保留队列事实并向驱动报告没有安全工作。
            return false;
        }
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
                crate::ui::widget_runtime::app_state::FocusRequest::Focus => {
                    if self.focus_target_available(id) {
                        self.set_keyboard_focus_visible(true);
                        self.set_focus(Some(id));
                    }
                }
                crate::ui::widget_runtime::app_state::FocusRequest::FocusAndReveal => {
                    if self.focus_target_available(id) {
                        self.set_keyboard_focus_visible(true);
                        self.set_focus(Some(id));
                        self.reveal_focused_target(id);
                    }
                }
                crate::ui::widget_runtime::app_state::FocusRequest::Blur => {
                    if self.managers.focus.focused_widget() == Some(id) {
                        self.set_focus(None);
                    }
                }
            }
        }
        self.app_state_focus_requests_scratch = requests;
        true
    }

    pub(crate) fn has_app_state_focus_requests(&self) -> bool {
        // 非运行态树不向窗口循环暴露无法安全执行的焦点工作。
        if !self.accepts_external_work() {
            // 防止旧焦点请求在 teardown 前形成忙循环。
            return false;
        }
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
            node.widget_snapshot(id),
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

    /// 分配属于本树作用域且带 generation 的新组件标识。
    pub fn alloc_id(&mut self) -> WidgetId {
        // 停止树不得再分配能够逃逸到调用方的新组件身份。
        assert!(self.accepts_coordination_work());
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

    /// 安装根组件及其直接子组件，并返回根组件标识。
    pub fn set_root_with_children(
        &mut self,
        widget: Box<dyn Widget>,
        children: Vec<Box<dyn Widget>>,
    ) -> WidgetId {
        let id = self.set_root(widget);
        for child in children {
            self.add_child(id, child);
        }
        id
    }

    /// 向父节点添加组件及其直接子组件，并返回新组件标识。
    pub fn add_child_with_children(
        &mut self,
        parent_id: WidgetId,
        widget: Box<dyn Widget>,
        children: Vec<Box<dyn Widget>>,
    ) -> WidgetId {
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

    /// 向全部节点发布主题变化，并重绘使用调色板的节点。
    pub fn notify_theme_changed(&mut self) {
        // fail-stop 后不得再次调用节点的主题生命周期。
        if !self.accepts_external_work() {
            // 外部主题通知在停止树上保持无副作用。
            return;
        }
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

    /// 返回当前根节点；空树或停止树返回空值。
    pub fn root(&self) -> Option<&BoxedWidget> {
        self.root_id.and_then(|id| self.get(id))
    }
    /// 返回当前根组件标识；空树或停止树返回空值。
    pub fn root_id(&self) -> Option<WidgetId> {
        // fail-stop 后不向公开调用方暴露半提交根身份。
        if !self.accepts_coordination_work() {
            // owner teardown 通过树核心私有字段完成，不依赖公开根访问。
            return None;
        }
        self.root_id
    }
    /// 返回当前根节点的可变借用；空树或停止树返回空值。
    pub fn root_mut(&mut self) -> Option<&mut BoxedWidget> {
        self.root_id.and_then(|id| self.get_mut(id))
    }

    /// 按树遍历顺序返回首个指定组件类型的标识。
    pub fn find_by_type<T: Widget + 'static>(&self) -> Option<WidgetId> {
        for &id in self.traverse().iter() {
            if let Some(node) = self.get(id) {
                if node.widget().as_any().downcast_ref::<T>().is_some() {
                    return Some(id);
                }
            }
        }
        None
    }

    /// 按树遍历顺序返回全部指定组件类型及其共享借用。
    pub fn find_all_by_type<T: Widget + 'static>(&self) -> Vec<(WidgetId, &T)> {
        let mut results = Vec::new();
        for &id in self.traverse().iter() {
            if let Some(node) = self.get(id) {
                if let Some(w) = node.widget().as_any().downcast_ref::<T>() {
                    results.push((id, w));
                }
            }
        }
        results
    }

    /// 在异常事务边界内修改首个指定类型的组件并返回其标识。
    pub fn find_by_type_and_modify<T: Widget + 'static>(
        &mut self,
        f: impl FnOnce(&mut T),
    ) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        // 公开用户修改闭包必须在异常边界内发布，panic 后不能重新开放半修改组件。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 调用用户闭包前线性化真实组件值即将被改写。
            tree.mark_coordination_publish_started();
            // 只在目标身份与类型仍有效时执行一次用户修改。
            if let Some(node) = tree.get_mut(id) {
                // 类型查询复用调用前已经确认的稳定组件身份。
                if let Some(w) = node.widget_mut().as_any_mut().downcast_mut::<T>() {
                    // 用户闭包异常会由外层事务把树置为 fail-stop。
                    f(w);
                }
            }
            // 保持原公开 API 返回已匹配的组件身份。
            Some(id)
        })
    }

    /// 使用树作用域、槽位与 generation 安全查询节点。
    pub fn get(&self, id: WidgetId) -> Option<&BoxedWidget> {
        // 停止树不得泄漏可继续调用用户组件的半提交节点引用。
        if !self.accepts_coordination_work() {
            // shutdown 使用树核心私有 raw accessor，不经过公开边界。
            return None;
        }
        // 运行态与协调态共享同一 generation 安全寻址实现。
        self.get_raw(id)
    }

    // 让树核心关闭路径访问已经停止但仍待释放的真实节点。
    pub(super) fn get_raw(&self, id: WidgetId) -> Option<&BoxedWidget> {
        // 先验证树作用域、槽位与 generation，再读取物理节点。
        let slot = self.node_slot_for(id)?;
        // 返回仅限 tree_core 资源释放使用的节点引用。
        self.nodes.get(slot).and_then(|n| n.as_ref())
    }
    /// 使用树作用域、槽位与 generation 安全查询节点的可变借用。
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut BoxedWidget> {
        // 停止树不得泄漏可直接执行用户组件方法的可变节点引用。
        if !self.accepts_coordination_work() {
            // 调用方必须丢弃旧树并创建新的 owner。
            return None;
        }
        // 运行态与协调态共享同一 generation 安全寻址实现。
        self.get_mut_raw(id)
    }

    // 让树核心关闭路径可变访问已经停止但仍待执行受控生命周期的节点。
    pub(super) fn get_mut_raw(&mut self, id: WidgetId) -> Option<&mut BoxedWidget> {
        // 先验证树作用域、槽位与 generation，再读取物理节点。
        let slot = self.node_slot_for(id)?;
        // 返回仅限 tree_core 关闭实现使用的可变节点引用。
        self.nodes.get_mut(slot).and_then(|n| n.as_mut())
    }

    /// 设置节点的兄弟绘制层级；节点不存在时保持无操作。
    pub fn set_z_index(&mut self, id: WidgetId, z: i32) -> &mut Self {
        if let Some(n) = self.get_mut(id) {
            n.set_z_index(z);
        }
        self
    }

    /// 事务化移除节点子树并按逆序执行组件生命周期。
    pub fn remove(&mut self, id: WidgetId) {
        // 公开移除会执行组件生命周期，必须统一进入 panic 事务边界。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 在当前事务内递归完成真实移除。
            tree.remove_in_transaction(id);
        });
    }

    // 在已建立事务的边界内递归移除节点子树。
    pub(super) fn remove_in_transaction(&mut self, id: WidgetId) {
        self.remove_in_transaction_inner(id, true);
    }

    // 递归后代由最外层移除根统一覆盖脏区与父级布局通知。
    fn remove_in_transaction_inner(&mut self, id: WidgetId, publish_parent_damage: bool) {
        // 节点移除会立即改写真实结构，停止树拒绝且协调事务记录发布事实。
        self.mark_coordination_publish_started();
        self.tree_version += 1;
        self.active_widget_animations.remove(&id);

        // 根边界已经包含全部后代；递归重复扫描会让整棵子树退化为平方复杂度。
        let old_visual_bounds = publish_parent_damage
            .then(|| self.visual_subtree_bounds(id))
            .flatten();
        // 普通视觉边界会跳过 overlay；移除根必须在所有者失效前保存旧浮层像素。
        let (removed_widget_overlay, old_overlay_bounds) = if publish_parent_damage {
            let mut removed = false;
            let mut bounds = Vec::new();
            for entry in self
                .overlay_stack
                .iter()
                .filter(|entry| !entry.is_managed())
            {
                if !self.is_descendant_of(entry.owner(), id) {
                    continue;
                }
                removed = true;
                if let Some(rect) = entry
                    .bounds_rect()
                    .filter(|rect| rect.w > 0.0 && rect.h > 0.0)
                {
                    bounds.push(rect);
                }
            }
            (removed, bounds)
        } else {
            (false, Vec::new())
        };

        let Some(slot) = self.node_slot_for(id) else {
            return;
        };
        let parent_id = self.nodes[slot].as_ref().and_then(|n| n.parent());
        if publish_parent_damage {
            // 根仍可寻址时一次性交付整棵子树的交互取消与逆序销毁生命周期。
            self.cancel_subtree_interaction(id);
            self.teardown_subtree(id);
        }
        if let Some(node) = self.nodes.get_mut(slot) {
            if let Some(node) = node.take() {
                // 节点已真实离开槽位后，取消其待提交项并立即释放实际动画源所有权。
                self.release_node_animated_sources_immediately(id);
                for child_id in node.children().to_vec() {
                    self.remove_in_transaction_inner(child_id, false);
                }
                self.handler_table.clear_widget(id);
                self.render_handler_table.clear_widget(id);
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
                self.managers.focus.unregister_widget(id);
                self.managers.interaction.unregister_widget(id);
                self.managers.drag.unregister_widget(id);
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

        if publish_parent_damage {
            if let Some(rect) = old_visual_bounds {
                if let Some(pid) = parent_id {
                    self.push_paint_invalidation(pid, Some(rect));
                }
            }
            if let Some(pid) = parent_id {
                for rect in old_overlay_bounds.iter().copied() {
                    self.push_paint_invalidation(pid, Some(rect));
                }
            }
            if removed_widget_overlay {
                // 浮层成员已经离开合成拓扑，强制重建目标以清除保留缓冲中的旧像素。
                self.mark_full_frame_composite();
            }
            if let Some(pid) = parent_id {
                self.push_layout_invalidation(pid);
                self.propagate_layout_invalidation(pid);
            }
        }
        // 事务外的实际移除（含 leave 结束）现在可释放无承载节点的私有状态。
        self.prune_widget_state_scopes_if_idle();
    }

    /// 事务化设置节点及其全部后代的可见性。
    pub fn set_visible(&mut self, id: WidgetId, visible: bool) {
        // 公开可见性传播会执行组件生命周期，统一进入 panic 事务边界。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 在同一事务内发布整棵子树的可见性变化。
            tree.set_visible_in_transaction(id, visible);
        });
    }

    // 在已建立事务的边界内发布递归可见性变化。
    fn set_visible_in_transaction(&mut self, id: WidgetId, visible: bool) {
        // 可见性传播会改写节点与生命周期，先进入统一发布门禁。
        self.mark_coordination_publish_started();
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
    pub fn traverse(&self) -> std::cell::Ref<'_, [WidgetId]> {
        // 停止树不得通过公开遍历泄漏半提交节点身份集合。
        if !self.accepts_coordination_work() {
            // 清空旧缓存，防止调用方观察 fail-stop 前留下的路径。
            self.cached_traversal.borrow_mut().0.clear();
            // 返回与正常签名一致的稳定空借用。
            return std::cell::Ref::map(self.cached_traversal.borrow(), |(ids, _)| {
                // 只暴露已经清空的身份切片。
                ids.as_slice()
            });
        }
        {
            let mut cache = self.cached_traversal.borrow_mut();
            let (ref mut ids, ref mut ver) = *cache;
            if *ver != self.tree_version {
                ids.clear();
                if let Some(root_id) = self.root_id {
                    // Iterative traversal avoids stack overflow on very deep trees.
                    let mut stack = self.traversal_stack_scratch.borrow_mut();
                    stack.clear();
                    stack.push(root_id);
                    while let Some(current) = stack.pop() {
                        ids.push(current);
                        if let Some(node) = self.get(current) {
                            for child_id in node.children().iter().rev() {
                                stack.push(*child_id);
                            }
                        }
                    }
                    stack.clear();
                }
                *ver = self.tree_version;
            }
        }
        std::cell::Ref::map(self.cached_traversal.borrow(), |(ids, _)| ids.as_slice())
    }

    /// 收集当前可调度定时器的树级工作键与延迟。
    pub fn active_timers(&mut self) -> Vec<(u64, std::time::Duration)> {
        // 停止树不得再向窗口调度任何用户定时器。
        if !self.accepts_external_work() {
            // 返回空集合以撤销驱动层的后续 timer 安排。
            return Vec::new();
        }
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
        // direct 调用也必须在 fail-stop 后拒绝触达用户节点。
        if !self.accepts_external_work() {
            // 停止树将定时器工作视为未处理。
            return EventResult::NotHandled;
        }
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

    /// 更新节点 frame，并在几何变化时请求布局与绘制失效传播。
    pub fn set_frame_dirty(&mut self, id: WidgetId, new_frame: Rect) {
        if !self.apply_frame_paint(id, new_frame) {
            return;
        }
        // 旧、新视觉子树已由 apply_frame_paint 提交，布局只需补充后代最终边界。
        self.note_frame_dirty_layout_root(id);
        self.push_layout_invalidation(id);
        self.propagate_layout_invalidation(id);
    }

    /// layout() 内写 frame：延后聚合 Paint，不重新入队 Layout。
    ///
    /// `set_frame_dirty` 会 `push_layout_invalidation`，若在收敛循环里调用，
    /// 会在结果已稳定后仍留下 Layout pending，下一帧无事件也再跑 layout（违反休眠）。
    pub(crate) fn set_layout_frame(
        &mut self,
        id: WidgetId,
        new_frame: Rect,
        damage: &mut tree_layout::LayoutFrameDamage,
    ) -> bool {
        #[cfg(test)]
        let before_h = self
            .get(id)
            .map(|n| n.frame().h.round() as i32)
            .unwrap_or(0);
        // 中间收敛状态不会上屏；归一并比较后只保存节点首次变化前的视觉边界。
        let new_frame = crate::ui::layout::engine::normalize_layout_rect(new_frame);
        match self.get(id) {
            Some(node) if node.frame() == new_frame => return false,
            None => return false,
            Some(_) => {}
        }
        if damage.seen.insert(id) {
            // 已记录的变化祖先会覆盖整棵子树，无需为每个后代重复扫描视觉边界。
            let mut ancestor = self.get(id).and_then(|node| node.parent());
            let mut covered = false;
            let mut old_bounds_covered = false;
            while let Some(ancestor_id) = ancestor {
                if damage.roots.contains(&ancestor_id) {
                    covered = true;
                    break;
                }
                if damage.frame_dirty_roots.contains(&ancestor_id) {
                    old_bounds_covered = true;
                    break;
                }
                ancestor = self.get(ancestor_id).and_then(|node| node.parent());
            }
            if !covered && !old_bounds_covered {
                damage.roots.insert(id);
                damage.entries.push((id, self.visual_subtree_bounds(id)));
            }
        }
        if let Some(node) = self.get_mut(id) {
            node.set_frame(new_frame);
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

    /// 布局收敛后一次性提交所有节点的首态与终态视觉脏区。
    pub(crate) fn flush_layout_frame_damage(
        &mut self,
        damage: &mut tree_layout::LayoutFrameDamage,
    ) {
        let full_paint = {
            let invalidation = self
                .invalidation
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            invalidation.paint_ids_into(&mut damage.prepainted);
            invalidation.needs_full_frame()
        };
        let mut entries = std::mem::take(&mut damage.entries);
        for (id, old_visual_bounds) in entries.drain(..) {
            let new_visual_bounds = self.visual_subtree_bounds(id);
            for rect in [old_visual_bounds, new_visual_bounds].into_iter().flatten() {
                if rect.w > 0.0 && rect.h > 0.0 {
                    self.push_paint_invalidation(id, Some(rect));
                }
            }
        }
        // 后代仍保持独立 Paint 身份；旧位置已由变化根的旧子树边界覆盖，
        // 因此这里只提交最终自身边界，避免把每个节点都扩成整页脏区。
        for id in damage.seen.iter().copied().filter(|id| {
            !full_paint && !damage.roots.contains(id) && !damage.prepainted.contains(id)
        }) {
            if let Some(rect) = self.visible_visual_rect_for(id) {
                self.push_paint_invalidation(id, Some(rect));
            }
        }
        damage.entries = entries;
        damage.seen.clear();
        damage.roots.clear();
        damage.prepainted.clear();
    }

    // 测试目标保留布局帧 trace 取出入口，供布局收敛测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn take_layout_frame_trace(&self) -> Vec<(u8, WidgetId, i32, i32)> {
        std::mem::take(&mut *self.layout_frame_trace.borrow_mut())
    }

    pub(crate) fn apply_frame_paint(&mut self, id: WidgetId, new_frame: Rect) -> bool {
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
