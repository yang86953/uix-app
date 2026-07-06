//! 閫傞厤灞?鈥?灏?View 鏍戝睍寮€涓?WidgetTree銆?//!
//! 鐢ㄦ埛鍦?`App::run()` 鍐呴儴閫氳繃鏈ā鍧楀皢鐢ㄦ埛灞傜殑 `View` 鏍戦€掑綊灞曞紑涓烘鏋跺眰鐨?//! `WidgetTree`锛屽畬鍏ㄩ殣钘?`WidgetNode`銆乣BoxedWidget` 绛夊唴閮ㄦ蹇点€?//!
//! # 鑱岃矗
//!
//! 1. `ViewAdapter::build(root)` 鈥?鍏ュ彛锛屽皢 `View` 鏋勫缓涓?`WidgetTree`銆?//!    鑷姩璁剧疆 View 涓婁笅鏂囷紝浣?`State::new` 缁戝畾鍒版纭殑 WidgetId銆?//! 2. `expand(node)` 鈥?閫掑綊灞曞紑锛歏iewNode 鈫?WidgetNode銆?//! 3. `apply_style(widget, style)` 鈥?灏?Style 搴旂敤鍒板叿浣撶粍浠剁被鍨嬨€?//!
//! # State 鑷姩鑴忔爣璁?//!
//! - View 鏋勫缓鏈燂細`begin_state_capture` 鎹曡幏 `State::new`锛坄capture_view` / `with_view_context`锛?//! - layout 鍚庯細`bind_reactive_widget_states` 鎺㈡祴 `dynamic_label` 闂寘渚濊禆骞剁粦瀹?Paint 澶辨晥
//! - 鍏滃簳锛歚bind_orphan_pending_states` 灏嗘湭鍏宠仈 State 缁戝埌鏍硅妭鐐?
use crate::ui::foundation::state::{begin_state_capture, end_state_capture};
use crate::ui::style::Style;
use crate::ui::traits::WidgetComponent;
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::{Button, Container, Grid, Input, Label};
use crate::ui::{WidgetCore, WidgetId, WidgetNode, WidgetTree};
use std::any::TypeId;
use std::collections::{HashMap, HashSet};

/// View tree adapter.
pub struct ViewAdapter;

impl ViewAdapter {
    /// Builds a View while capturing State bindings.
    pub fn capture_view(view: impl View) -> ViewNode {
        begin_state_capture();
        let node = view.build();
        end_state_capture();
        node
    }

    /// Builds a View tree into a WidgetTree.
    pub fn build(view: impl View) -> WidgetTree {
        Self::build_nodes(Self::capture_view(view))
    }

    /// Builds an already expanded ViewNode tree into a WidgetTree.
    pub fn build_nodes(root: ViewNode) -> WidgetTree {
        let mut tree = WidgetTree::new();
        let wnode = Self::expand(root);
        tree.build(wnode);
        tree.bind_orphan_pending_states();
        tree.bind_pending_effects();
        tree
    }

    /// Reconciles a new View tree into an existing WidgetTree.
    pub fn reconcile(tree: &mut WidgetTree, view: impl View) {
        Self::reconcile_nodes(tree, Self::capture_view(view));
    }

    /// Reconciles an already captured ViewNode tree into an existing WidgetTree.
    pub fn reconcile_nodes(tree: &mut WidgetTree, root: ViewNode) {
        match tree.root_id() {
            Some(root_id) if Self::can_reuse(tree, root_id, &root) => {
                Self::reconcile_existing(tree, root_id, root);
            }
            _ => {
                tree.build(Self::expand(root));
            }
        }
        tree.bind_orphan_pending_states();
        tree.bind_pending_effects();
    }

    /// Expands a ViewNode recursively into a WidgetNode.
    pub(crate) fn expand(node: ViewNode) -> WidgetNode {
        let children: Vec<WidgetNode> = node.children.into_iter().map(Self::expand).collect();

        let widget = Self::apply_style(node.widget, &node.style);

        let mut wnode = if children.is_empty() {
            WidgetNode::leaf(widget)
        } else {
            WidgetNode::new(widget, children)
        };

        if let Some(key) = node.key {
            wnode = wnode.key(&key);
        }

        if node.z_index != 0 {
            wnode = wnode.z_index(node.z_index);
        }

        if !node.handlers.is_empty() {
            wnode = wnode.with_handlers(node.handlers);
        }

        wnode
    }

    fn apply_style(
        mut widget: Box<dyn WidgetComponent>,
        style: &Style,
    ) -> Box<dyn WidgetComponent> {
        if style == &Style::default() {
            return widget;
        }

        let tid = widget.as_any().type_id();

        if tid == std::any::TypeId::of::<Container>() {
            if let Some(c) = widget.as_any_mut().downcast_mut::<Container>() {
                c.style = style.clone();
            }
        } else if tid == std::any::TypeId::of::<Label>() {
            if let Some(l) = widget.as_any_mut().downcast_mut::<Label>() {
                l.style = Some(style.clone());
            }
        } else if tid == std::any::TypeId::of::<Button>() {
            if let Some(b) = widget.as_any_mut().downcast_mut::<Button>() {
                b.style = style.clone();
            }
        } else if tid == std::any::TypeId::of::<Grid>() {
            if let Some(g) = widget.as_any_mut().downcast_mut::<Grid>() {
                g.apply_style(style);
            }
        }

        widget
    }

    fn can_reuse(tree: &WidgetTree, id: WidgetId, node: &ViewNode) -> bool {
        tree.get(id)
            .is_some_and(|current| current.component().as_any().type_id() == node.widget_type_id())
    }

    fn reconcile_existing(tree: &mut WidgetTree, id: WidgetId, node: ViewNode) {
        let ViewNode {
            widget,
            children,
            style,
            z_index,
            key,
            handlers,
        } = node;
        let widget = Self::apply_style(widget, &style);
        Self::patch_widget(tree, id, widget);

        if let Some(current) = tree.get_mut(id) {
            current.set_key(key.map(Into::into));
            if current.z_index() != z_index {
                current.set_z_index(z_index);
            }
        }

        tree.handler_table().clear_component(id);
        for handler in handlers {
            tree.handler_table().register(id, handler);
        }

        tree.invalidate_paint(id);
        tree.push_layout_invalidation(id);
        tree.propagate_layout_invalidation(id);
        Self::reconcile_children(tree, id, children);
    }

    fn patch_widget(tree: &mut WidgetTree, id: WidgetId, widget: Box<dyn WidgetComponent>) {
        let Some(current) = tree.get_mut(id) else {
            return;
        };

        let next_type = widget.as_any().type_id();
        if current.component().as_any().type_id() != next_type {
            current.replace_component(widget);
            return;
        }

        if next_type == TypeId::of::<Container>() {
            if let Some(next) = widget.as_any().downcast_ref::<Container>() {
                if let Some(existing) = current
                    .component_mut()
                    .as_any_mut()
                    .downcast_mut::<Container>()
                {
                    existing.style = next.style.clone();
                    return;
                }
            }
            current.replace_component(widget);
            return;
        }

        if next_type == TypeId::of::<Label>() {
            let Ok(next) = widget.into_any().downcast::<Label>() else {
                return;
            };
            if let Some(existing) = current.component_mut().as_any_mut().downcast_mut::<Label>() {
                existing.sync_from(*next);
            }
            return;
        }

        if next_type == TypeId::of::<Button>() {
            let Ok(next) = widget.into_any().downcast::<Button>() else {
                return;
            };
            if let Some(existing) = current
                .component_mut()
                .as_any_mut()
                .downcast_mut::<Button>()
            {
                existing.sync_from(*next);
            }
            return;
        }

        if next_type == TypeId::of::<Input>() {
            let Ok(next) = widget.into_any().downcast::<Input>() else {
                return;
            };
            if let Some(existing) = current.component_mut().as_any_mut().downcast_mut::<Input>() {
                existing.sync_from(*next);
            }
            return;
        }

        if next_type == TypeId::of::<Grid>() {
            let Ok(next) = widget.into_any().downcast::<Grid>() else {
                return;
            };
            current.replace_component(next);
            return;
        }

        current.replace_component(widget);
    }

    fn reconcile_children(tree: &mut WidgetTree, parent_id: WidgetId, children: Vec<ViewNode>) {
        let old_children = tree
            .get(parent_id)
            .map(|node| node.children().to_vec())
            .unwrap_or_default();
        let mut old_by_key: HashMap<String, WidgetId> = HashMap::new();
        for &child_id in &old_children {
            if let Some(key) = tree.get(child_id).and_then(|node| node.key()) {
                old_by_key.insert(key.to_string(), child_id);
            }
        }

        let mut used_old = HashSet::new();
        let mut new_order = Vec::with_capacity(children.len());

        for (index, child) in children.into_iter().enumerate() {
            let candidate = child
                .key
                .as_ref()
                .and_then(|key| old_by_key.get(key).copied())
                .filter(|id| !used_old.contains(id))
                .or_else(|| {
                    if child.key.is_some() {
                        return None;
                    }
                    old_children
                        .get(index)
                        .copied()
                        .filter(|id| !used_old.contains(id))
                        .filter(|id| tree.get(*id).is_some_and(|node| node.key().is_none()))
                });

            let child_id = if let Some(child_id) = candidate {
                used_old.insert(child_id);
                if Self::can_reuse(tree, child_id, &child) {
                    Self::reconcile_existing(tree, child_id, child);
                    child_id
                } else {
                    tree.remove(child_id);
                    tree.build_child_node(parent_id, Self::expand(child))
                }
            } else {
                tree.build_child_node(parent_id, Self::expand(child))
            };
            new_order.push(child_id);
        }

        for child_id in old_children {
            if !used_old.contains(&child_id) && tree.get(child_id).is_some() {
                tree.remove(child_id);
            }
        }

        let order_changed = tree
            .get(parent_id)
            .is_some_and(|parent| parent.children() != new_order.as_slice());
        if order_changed {
            if let Some(parent) = tree.get_mut(parent_id) {
                *parent.children_mut() = new_order;
            }
            tree.tree_version += 1;
        }
    }
}

/// Runs a closure inside the current View state-capture context.
pub fn with_view_context<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    begin_state_capture();
    let result = f();
    end_state_capture();
    result
}

#[cfg(test)]
#[path = "../../tests/ui/view/adapter.rs"]
mod tests;

