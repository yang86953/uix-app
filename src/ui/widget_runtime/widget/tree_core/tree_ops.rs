use super::*;
use crate::ui::adapter::ViewAdapter;
use crate::ui::widget_runtime::provider_context::{
    ProviderContext, current_provider_context, with_provider_context,
};

impl WidgetTree {
    pub(crate) fn root_bootstrap_constraints() -> Constraints {
        Constraints::loose(Self::ROOT_BOOTSTRAP_SIZE)
    }

    /// 拆除现有树，并将组件及其构建出的后代安装为新根。
    pub fn set_root(&mut self, widget: Box<dyn Widget>) -> WidgetId {
        // 公开换根会调用用户 build 与生命周期，必须统一进入 panic 事务边界。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 事务内部使用不会再次建立独立提交点的核心实现。
            tree.set_root_with_context(widget, current_provider_context(), true)
        })
    }

    pub(super) fn set_root_with_context(
        &mut self,
        widget: Box<dyn Widget>,
        provider_context: ProviderContext,
        include_view_children: bool,
    ) -> WidgetId {
        // 换根会立即拆除既有运行时树，先线性化不可回滚发布。
        self.mark_coordination_publish_started();
        if let Some(root) = self.root_id {
            self.cancel_subtree_interaction(root);
        }
        self.teardown_all();
        // 完整换根会销毁全部运行时节点，先逐个释放它们的动画源所有权。
        self.clear_node_animated_source_owners();

        // Hard reset: clear the old tree and invalidate every previous WidgetId.
        self.nodes.clear();
        self.free_slots.clear();
        for generation in &mut self.generations {
            *generation = generation.wrapping_add(1);
        }
        self.next_slot = 0;
        self.root_id = None;
        self.handler_table.clear();
        self.render_handler_table.clear();
        self.overlay_stack.clear();
        self.active_widget_animations.clear();
        self.focus_handles.clear();
        self.reset_interaction_state();
        self.tree_version += 1;

        let children = with_provider_context(&provider_context, || widget.build());
        let view_children = include_view_children.then(|| {
            with_provider_context(&provider_context, || {
                crate::ui::adapter::view_children(widget.as_ref())
            })
        });
        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new_with_context(widget, provider_context.clone());
        boxed.set_id(id);
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
            self.add_child_with_context(id, child, provider_context.clone(), true);
        }
        for child in view_children.into_iter().flatten() {
            self.build_node(ViewAdapter::expand(child), Some(id));
        }
        // 公开直接换根不会经过 build_node，仅在该路径补做 Calendar 首次动态物化。
        if include_view_children {
            // 此时 root 已注册为真实 Transfer owner，可安全捕获自定义条目。
            self.refresh_transfer_item_widget(id);
            // 此时 root 已注册为真实 Carousel owner，可安全捕获自定义箭头。
            self.refresh_carousel_custom_arrows_widget(id);
            // 此时 root 已注册为真实 owner，动态捕获可以安全绑定树私有 store。
            self.refresh_calendar_cell_widget(id);
            // 导航 capability 启用时也物化 root Anchor 的首次动态容器。
            #[cfg(feature = "navigation")]
            // 此时 root 已是 live owner，捕获能力绑定本树私有状态存储。
            self.refresh_anchor_container_widget(id);
        }
        self.push_layout_invalidation(id);
        id
    }

    /// 将组件及其构建出的后代添加到指定父节点下。
    pub fn add_child(&mut self, parent_id: WidgetId, child: Box<dyn Widget>) -> WidgetId {
        let provider_context = self
            .get(parent_id)
            .map(|parent| parent.provider_context().clone())
            .unwrap_or_else(current_provider_context);
        // 公开加子节点会调用用户 build、attach 与父节点通知，统一捕获发布异常。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 事务内部直接调用核心插入实现，避免产生第二个线性化点。
            tree.add_child_with_context(parent_id, child, provider_context, true)
        })
    }

    pub(super) fn add_child_with_context(
        &mut self,
        parent_id: WidgetId,
        child: Box<dyn Widget>,
        provider_context: ProviderContext,
        include_view_children: bool,
    ) -> WidgetId {
        // 插入子节点会改写真实槽位与父子结构，先记录发布事实。
        self.mark_coordination_publish_started();
        self.tree_version += 1;
        let children = with_provider_context(&provider_context, || child.build());
        let view_children = include_view_children.then(|| {
            with_provider_context(&provider_context, || {
                crate::ui::adapter::view_children(child.as_ref())
            })
        });
        let child_id = self.alloc_id();
        let mut boxed = BoxedWidget::new_with_context(child, provider_context.clone());
        boxed.set_id(child_id);
        boxed.set_parent(Some(parent_id));
        let child_slot = child_id.slot();
        if self.nodes.len() <= child_slot {
            self.nodes.resize_with(child_slot + 1, || None);
        }
        self.nodes[child_slot] = Some(boxed);
        self.register_focusable(child_id);
        self.attach_node(child_id);
        if let Some(parent) = self.get_mut(parent_id) {
            // 先把新节点纳入父节点的直接子节点集合。
            parent.children_mut().push(child_id);
            // 再让父组件基于完整的新集合同步派生运行态。
            parent.notify_children_changed();
        }
        for child in children {
            self.add_child_with_context(child_id, child, provider_context.clone(), true);
        }
        for child in view_children.into_iter().flatten() {
            self.build_node(ViewAdapter::expand(child), Some(child_id));
        }
        // 公开直接加子节点不会经过 build_node，仅在该路径补做 Calendar 首次动态物化。
        if include_view_children {
            // 此时 child 已连接父树，可安全捕获 Transfer 自定义条目。
            self.refresh_transfer_item_widget(child_id);
            // 此时 child 已连接父树，可安全捕获 Carousel 自定义箭头。
            self.refresh_carousel_custom_arrows_widget(child_id);
            // 此时 child 已连接父树，动态捕获与离场判断均使用真实 owner。
            self.refresh_calendar_cell_widget(child_id);
            // 导航 capability 启用时也物化直接追加 Anchor 的首次动态容器。
            #[cfg(feature = "navigation")]
            // 此时 child 已连接父树，捕获与离场检查使用真实 owner。
            self.refresh_anchor_container_widget(child_id);
        }

        // 结构变化：Layout 失效向上传播。
        self.push_layout_invalidation(parent_id);
        self.propagate_layout_invalidation(parent_id);

        child_id
    }

    pub(crate) fn set_node_visibility(&mut self, id: WidgetId, visible: bool) {
        let changed = self
            .get(id)
            .is_some_and(|node| node.visibility_gate() != visible);
        if !changed {
            return;
        }
        // 生命周期可见性切换可能执行用户回调，统一进入树事务异常边界。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 事务内部执行唯一一次真实可见性发布。
            tree.set_node_visibility_in_transaction(id, visible);
        });
    }

    // 在已建立事务的边界内发布节点可见性变化。
    fn set_node_visibility_in_transaction(&mut self, id: WidgetId, visible: bool) {
        // 可见性会立即改写节点、交互与失效队列，先记录发布事实。
        self.mark_coordination_publish_started();
        if !visible {
            self.cancel_subtree_interaction(id);
        }
        if let Some(node) = self.get_mut(id) {
            node.set_visible(visible);
        }
        self.tree_version += 1;
        self.invalidate_paint(id);
        self.push_layout_invalidation(id);
        self.propagate_layout_invalidation(id);
    }

    // WidgetNode tree building.

    /// 使用声明式节点及其后代重建组件树。
    pub fn build(&mut self, node: WidgetNode) -> WidgetId {
        // 公开 WidgetNode 建树仍可能调用组件生命周期与动态 renderer。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 整树构建会替换根与节点结构，先记录不可回滚发布事实。
            tree.mark_coordination_publish_started();
            // 直接重建整树前释放旧根持有的结构性 State 租约。
            tree.root_reconcile_state_binds.clear();
            // 在同一事务内递归构建完整节点树。
            tree.build_node(node, None)
        })
    }

    pub(crate) fn build_child_node(&mut self, parent_id: WidgetId, node: WidgetNode) -> WidgetId {
        // crate 内动态入口也必须共享相同 panic 边界，防止未来调用者漏包事务。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 在当前或嵌套事务内递归建立子树。
            tree.build_node(node, Some(parent_id))
        })
    }

    /// 用新的声明式节点列表替换指定父节点的全部子树。
    pub fn set_children(&mut self, parent_id: WidgetId, children: Vec<WidgetNode>) {
        // 公开子列表替换可能触发生命周期与 renderer，统一建立事务边界。
        self.with_widget_state_transaction(Vec::new(), |tree| {
            // 在同一事务中完成全部旧子节点移除与新子树构建。
            tree.set_children_in_transaction(parent_id, children);
        });
    }

    // 在既有事务内完成子列表的不可回滚发布。
    fn set_children_in_transaction(&mut self, parent_id: WidgetId, children: Vec<WidgetNode>) {
        // 子列表替换会移除并重建真实节点，先记录不可回滚发布事实。
        self.mark_coordination_publish_started();
        let old_children: Vec<WidgetId> = self
            .get(parent_id)
            .map(|n| n.children().to_vec())
            .unwrap_or_default();
        for &cid in &old_children {
            self.remove_in_transaction(cid);
        }
        self.tree_version += 1;
        for child in children {
            self.build_node(child, Some(parent_id));
        }
    }

    fn build_node(&mut self, node: WidgetNode, parent: Option<WidgetId>) -> WidgetId {
        // 节点建造最终会注册真实节点与 sidecar，先记录发布事实。
        self.mark_coordination_publish_started();
        let WidgetNode {
            widget,
            children,
            provider_context,
            visible,
            visual_transform,
            position,
            user_select,
            cursor,
            enter_animation,
            enter_deadline,
            leave_animation,
            declared_opacity,
            z_index,
            key,
            automation_id,
            tab_idx,
            tab_index_override,
            focus_handle,
            accessibility_override,
            handlers,
            system_event_handlers,
            render_handlers,
            captured_state_binds,
            scoped_rebuild,
            captured_effects,
            uix_widget_scopes,
            animated_sources,
        } = node;
        let id = match parent {
            Some(parent) => self.add_child_with_context(parent, widget, provider_context, false),
            None => self.set_root_with_context(widget, provider_context, false),
        };
        let widget_view_children = self
            .get(id)
            .map(|current| {
                let provider_context = current.provider_context().clone();
                with_provider_context(&provider_context, || {
                    crate::ui::adapter::view_children(current.widget())
                })
            })
            .unwrap_or_default();
        if let Some(node) = self.get_mut(id) {
            node.set_visible(visible);
            node.set_visual_transform(visual_transform);
            // 在首次布局前安装节点完整定位声明。
            node.set_position(position);
            // 先安装节点声明，随后结合真实父链解析 used-value。
            node.set_declared_user_select(user_select);
            // 把声明节点的可继承光标覆盖安装到运行时节点。
            node.set_cursor(cursor);
            node.set_enter_animation(enter_animation, enter_deadline);
            node.set_leave_animation(leave_animation);
            // 声明透明度与过渡覆盖层在合成边界叠乘，进入统一节点合成通道。
            node.set_declared_opacity(declared_opacity);
            node.set_key(key);
            node.set_automation_id(automation_id);
            node.set_z_index(z_index);
            let tab_index_override =
                tab_index_override.or_else(|| (tab_idx != 0).then_some(tab_idx));
            if let Some(tab_index) = tab_index_override {
                node.set_tab_index(tab_index);
            } else {
                node.set_tab_index_override(None);
            }
            node.set_accessibility_override(accessibility_override);
            // 把成功挂载节点拥有的 Effect 与其真实生命周期绑定。
            node.replace_captured_effects(captured_effects);
            // 保存声明展开携带的全部组件私有状态作用域。
            node.set_uix_widget_scopes(uix_widget_scopes);
        }
        // 在构建任何后代前让当前节点取得稳定有效选择策略。
        self.set_node_user_select(id, user_select);
        self.register_focusable(id);
        // 非根节点已加入当前树后才由自身生命周期持有结构性 State 租约。
        if parent.is_some() {
            // 作用域节点的结构依赖安装为节点作用域失效；其余维持整树请求。
            if let Some(rebuild) = scoped_rebuild {
                self.install_scoped_node(id, captured_state_binds, rebuild);
            } else {
                // 把结构性 State 订阅绑定到该实际节点的真实移除生命周期。
                self.replace_node_captured_state_binds(id, captured_state_binds);
            }
        }
        self.set_focus_handle(id, focus_handle);
        let handler_signatures = handlers
            .iter()
            .map(|handler| handler.authored_signature())
            .collect();
        if let Some(node) = self.get_mut(id) {
            node.set_handler_signatures(handler_signatures);
            node.replace_system_event_handlers(system_event_handlers);
        }
        for handler in handlers {
            self.handler_table.register(id, handler);
        }
        self.render_handler_table
            .replace_widget(id, render_handlers);
        // 初建 authored slide 不得占用 Carousel 固定动态箭头的保留 key。
        assert!(
            // 非 Carousel 节点不受该专属身份约束。
            !self.is_carousel_custom_arrows_widget(id)
                // Carousel 的全部 authored 直接子节点必须避开框架固定 key。
                || children.iter().all(|child| {
                    // 只比较同一父级 keyed 协调使用的直接根 key。
                    child.key.as_deref()
                        // 拒绝与动态箭头命名空间相同的 authored 身份。
                        != Some(
                            // 引用产品唯一公开给 crate 的固定身份常量。
                            crate::ui::widgets::display::Carousel::CUSTOM_ARROWS_CHILD_KEY,
                        )
                }),
            // 在执行任何箭头工厂前给出稳定冲突诊断。
            "Carousel authored child 不得使用保留 key uix:carousel:custom-arrows"
        );
        // 导航 capability 启用时拒绝初建 authored child 占用 Anchor 容器保留 key。
        #[cfg(feature = "navigation")]
        assert!(
            // 非 Anchor 节点不受该专属动态容器身份约束。
            !self.is_anchor_container_widget(id)
                // Anchor 的声明直接子节点不得伪装为框架动态容器。
                || children.iter().all(|child| {
                    // 只比较同一父级 keyed 协调使用的直接根 key。
                    child.key.as_deref()
                        != Some(crate::ui::widgets::navigation::Anchor::CONTAINER_CHILD_KEY)
                }),
            // 让初建与父级 reconcile 使用同一明确的失败语义。
            "Anchor authored child 不得使用保留 key uix:anchor:container"
        );
        for child in children {
            self.build_node(child, Some(id));
        }
        for child in widget_view_children {
            self.build_node(ViewAdapter::expand(child), Some(id));
        }
        self.refresh_virtual_scroll_widget(id, None);
        // 表格 capability 启用时才刷新泛型单元格子树。
        #[cfg(feature = "table")]
        self.refresh_table_cell_widget(id);
        // 表格 capability 启用时才刷新扩展行子树。
        #[cfg(feature = "table")]
        self.refresh_table_expand_widget(id);
        self.refresh_select_option_widget(id);
        // Transfer 注册为真实 owner 后立即物化自定义条目并交接私有运行时输出。
        self.refresh_transfer_item_widget(id);
        // Carousel 注册为真实 owner 后立即物化固定自定义箭头。
        self.refresh_carousel_custom_arrows_widget(id);
        // Calendar 注册为真实 owner 后立即物化日期格，避免初建路径绕过动态状态捕获。
        self.refresh_calendar_cell_widget(id);
        // 导航 capability 启用时在 authored children 挂载后物化 Anchor 动态容器。
        #[cfg(feature = "navigation")]
        // 初建路径使用窄追加，绝不以仅含容器的 reconcile 吞掉 authored children。
        self.refresh_anchor_container_widget(id);
        // 节点及其所有递归子树成功建立后，才提交该节点捕获的动画源所有权。
        self.replace_node_animated_sources(id, animated_sources);
        id
    }
}
