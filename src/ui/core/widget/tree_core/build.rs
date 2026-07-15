use super::*;
use crate::ui::foundation::provider_context::{
    current_provider_context, with_provider_context, ProviderContext,
};

impl WidgetTree {
    pub(crate) fn root_bootstrap_constraints() -> Constraints {
        Constraints::loose(Self::ROOT_BOOTSTRAP_SIZE)
    }

    pub fn set_root(&mut self, widget: Box<dyn WidgetComponent>) -> ComponentId {
        self.set_root_with_context(widget, current_provider_context())
    }

    pub(super) fn set_root_with_context(
        &mut self,
        widget: Box<dyn WidgetComponent>,
        provider_context: ProviderContext,
    ) -> ComponentId {
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
        self.managers.clear_overrides();
        self.reset_interaction_state();
        self.tree_version += 1;

        let children = with_provider_context(&provider_context, || widget.build());
        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new_with_context(widget, provider_context.clone());
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
            self.add_child_with_context(id, child, provider_context.clone());
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
        self.add_child_with_context(parent_id, child, provider_context)
    }

    pub(super) fn add_child_with_context(
        &mut self,
        parent_id: ComponentId,
        child: Box<dyn WidgetComponent>,
        provider_context: ProviderContext,
    ) -> ComponentId {
        self.tree_version += 1;
        let children = with_provider_context(&provider_context, || child.build());
        let child_id = self.alloc_id();
        let mut boxed = BoxedWidget::new_with_context(child, provider_context.clone());
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
            self.add_child_with_context(child_id, child, provider_context.clone());
        }

        // 结构变化：Layout 失效向上传播。
        self.push_layout_invalidation(parent_id);
        self.propagate_layout_invalidation(parent_id);

        child_id
    }
}
