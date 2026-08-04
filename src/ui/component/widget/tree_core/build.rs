use super::*;
use crate::ui::adapter::ViewAdapter;
use crate::ui::component::provider_context::{
    current_provider_context, with_provider_context, ProviderContext,
};

impl WidgetTree {
    pub(crate) fn root_bootstrap_constraints() -> Constraints {
        Constraints::loose(Self::ROOT_BOOTSTRAP_SIZE)
    }

    pub fn set_root(&mut self, widget: Box<dyn WidgetComponent>) -> ComponentId {
        self.set_root_with_context(widget, current_provider_context(), true)
    }

    pub(super) fn set_root_with_context(
        &mut self,
        widget: Box<dyn WidgetComponent>,
        provider_context: ProviderContext,
        include_view_children: bool,
    ) -> ComponentId {
        if let Some(root) = self.root_id {
            self.cancel_subtree_interaction(root);
        }
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
        self.render_handler_table.clear();
        self.overlay_stack.clear();
        self.active_component_animations.clear();
        self.managers.clear_overrides();
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
        self.push_layout_invalidation(id);
        id
    }

    pub fn add_child(
        &mut self,
        parent_id: ComponentId,
        child: Box<dyn WidgetComponent>,
    ) -> ComponentId {
        let provider_context = self
            .get(parent_id)
            .map(|parent| parent.provider_context().clone())
            .unwrap_or_else(current_provider_context);
        self.add_child_with_context(parent_id, child, provider_context, true)
    }

    pub(super) fn add_child_with_context(
        &mut self,
        parent_id: ComponentId,
        child: Box<dyn WidgetComponent>,
        provider_context: ProviderContext,
        include_view_children: bool,
    ) -> ComponentId {
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
            parent.children_mut().push(child_id);
        }
        for child in children {
            self.add_child_with_context(child_id, child, provider_context.clone(), true);
        }
        for child in view_children.into_iter().flatten() {
            self.build_node(ViewAdapter::expand(child), Some(child_id));
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
            provider_context,
            visible,
            visual_transform,
            enter_animation,
            enter_deadline,
            leave_animation,
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
        } = node;
        let id = match parent {
            Some(parent) => self.add_child_with_context(parent, widget, provider_context, false),
            None => self.set_root_with_context(widget, provider_context, false),
        };
        let component_view_children = self
            .get(id)
            .map(|current| {
                let provider_context = current.provider_context().clone();
                with_provider_context(&provider_context, || {
                    crate::ui::adapter::view_children(current.component())
                })
            })
            .unwrap_or_default();
        if let Some(node) = self.get_mut(id) {
            node.set_visible(visible);
            node.set_visual_transform(visual_transform);
            node.set_enter_animation(enter_animation, enter_deadline);
            node.set_leave_animation(leave_animation);
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
        }
        self.register_focusable(id);
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
            .replace_component(id, render_handlers);
        for child in children {
            self.build_node(child, Some(id));
        }
        for child in component_view_children {
            self.build_node(ViewAdapter::expand(child), Some(id));
        }
        self.refresh_virtual_scroll_component(id, None);
        // 表格 capability 启用时才刷新泛型单元格子树。
        #[cfg(feature = "table")]
        self.refresh_table_cell_component(id);
        // 表格 capability 启用时才刷新扩展行子树。
        #[cfg(feature = "table")]
        self.refresh_table_expand_component(id);
        self.refresh_select_option_component(id);
        id
    }
}
