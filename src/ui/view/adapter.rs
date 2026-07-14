//! ViewAdapter - expands a View tree into a WidgetTree.
//!
//! `App::run()` uses this module to recursively expand user-authored `View`
//! trees into framework `WidgetTree` nodes, keeping `WidgetNode` and
//! `BoxedWidget` internal.
//!
//! # Responsibilities
//!
//! 1. `ViewAdapter::build(root)` captures view context and builds a `WidgetTree`.
//! 2. `expand(node)` recursively converts `ViewNode` into `WidgetNode`.
//! 3. `apply_style(widget, style)` applies declarative style to concrete widgets.
//!
//! # State Binding
//!
//! - During view build, `begin_state_capture` records `State::new` instances.
//! - After layout, `bind_reactive_widget_states` detects dynamic label closure
//!   dependencies and binds them to narrow Paint invalidation.
//! - As a fallback, `bind_orphan_pending_states` binds unassociated build-time
//!   `State::get()` dependencies to reconcile (structural View updates).
//! - render 期读取的 State / Computed 绑定窄 Paint；DynamicLabel 还会在 layout 后
//!   主动探测闭包依赖。
use crate::ui::component_patch::{builtin_widget_runtime_changed, patch_builtin_widget};
use crate::ui::component_snapshot::SnapshotFields;
use crate::ui::core::widget::{WidgetCore, WidgetNode};
use crate::ui::event::{HandlerRegistration, HandlerSignature, SemanticKind};
use crate::ui::foundation::state::{begin_state_capture, end_state_capture};
use crate::ui::style::Style;
use crate::ui::traits::WidgetComponent;
use crate::ui::view::{View, ViewNode};
use crate::ui::widgets::{Button, Container, Grid, Label};
use crate::ui::window_chrome::WindowInteractionRegion;
use crate::ui::{ComponentId, WidgetTree};
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

    /// Builds a ViewNode while capturing State bindings.
    pub fn capture_root<F>(build_root: F) -> ViewNode
    where
        F: FnOnce() -> ViewNode,
    {
        begin_state_capture();
        let node = build_root();
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

        let widget = Self::apply_style(
            node.widget,
            &node.style,
            node.flex_grow_override,
            node.flex_shrink_override,
        );

        let mut wnode = if children.is_empty() {
            WidgetNode::leaf(widget)
        } else {
            WidgetNode::new(widget, children)
        };

        if let Some(key) = node.key {
            wnode = wnode.key(&key);
        }

        if let Some(automation_id) = node.automation_id {
            wnode = wnode.automation_id(&automation_id);
        }

        if node.z_index != 0 {
            wnode = wnode.z_index(node.z_index);
        }

        if !node.handlers.is_empty() {
            wnode = wnode.with_handlers(node.handlers);
        }

        if !node.render_handlers.is_empty() {
            wnode = wnode.with_render_handlers(node.render_handlers);
        }

        wnode
    }

    pub(crate) fn apply_style(
        mut widget: Box<dyn WidgetComponent>,
        style: &Style,
        flex_grow_override: Option<f32>,
        flex_shrink_override: Option<f32>,
    ) -> Box<dyn WidgetComponent> {
        let style_is_default = style == &Style::default();
        if style_is_default && flex_grow_override.is_none() && flex_shrink_override.is_none() {
            return widget;
        }

        let tid = widget.as_any().type_id();

        if tid == std::any::TypeId::of::<Container>() {
            if let Some(c) = widget.as_any_mut().downcast_mut::<Container>() {
                if !style_is_default {
                    c.style = c.style.clone().apply(style.clone());
                }
                // View DSL 显式 flex 覆盖（含 0.0），Style::apply 无法表达「设为默认值」
                if let Some(g) = flex_grow_override {
                    c.style.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    c.style.flex_shrink = s;
                }
            }
        } else if tid == std::any::TypeId::of::<Label>() {
            if let Some(l) = widget.as_any_mut().downcast_mut::<Label>() {
                let mut merged = l.style.clone().unwrap_or_default().apply(style.clone());
                if let Some(g) = flex_grow_override {
                    merged.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    merged.flex_shrink = s;
                }
                // ViewNode width/height → Label 固定尺寸（section 色条等）
                if let Some(w) = style.width {
                    l.fixed_width = Some(w);
                }
                if let Some(h) = style.height {
                    l.fixed_height = Some(h);
                }
                l.style = Some(merged);
            }
        } else if tid == std::any::TypeId::of::<Button>() {
            if let Some(b) = widget.as_any_mut().downcast_mut::<Button>() {
                b.style = style.clone();
                if let Some(g) = flex_grow_override {
                    b.style.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    b.style.flex_shrink = s;
                }
            }
        } else if tid == std::any::TypeId::of::<Grid>() {
            if let Some(g) = widget.as_any_mut().downcast_mut::<Grid>() {
                g.apply_style(style);
            }
        } else if tid == std::any::TypeId::of::<crate::ui::view::combinators::DynamicLabel>() {
            if let Some(dl) = widget
                .as_any_mut()
                .downcast_mut::<crate::ui::view::combinators::DynamicLabel>()
            {
                dl.set_style(style.clone());
            }
        } else if tid == std::any::TypeId::of::<WindowInteractionRegion>() {
            if let Some(region) = widget
                .as_any_mut()
                .downcast_mut::<WindowInteractionRegion>()
            {
                region.apply_view_style(style, flex_grow_override, flex_shrink_override);
            }
        }

        widget
    }

    fn can_reuse(tree: &WidgetTree, id: ComponentId, node: &ViewNode) -> bool {
        tree.get(id)
            .is_some_and(|current| current.component().as_any().type_id() == node.widget_type_id())
    }

    fn reconcile_existing(tree: &mut WidgetTree, id: ComponentId, node: ViewNode) {
        let ViewNode {
            widget,
            children,
            style,
            flex_grow_override,
            flex_shrink_override,
            z_index,
            key,
            automation_id,
            handlers,
            render_handlers,
        } = node;
        let widget = Self::apply_style(widget, &style, flex_grow_override, flex_shrink_override);
        let widget_changed = Self::patch_widget(tree, id, widget);

        let mut paint_changed = widget_changed;
        let mut layout_changed = widget_changed;
        if let Some(current) = tree.get_mut(id) {
            let next_key = key.map(Into::into);
            if current.key() != next_key.as_deref() {
                current.set_key(next_key);
            }
            let next_automation_id = automation_id.map(Into::into);
            if current.automation_id() != next_automation_id.as_deref() {
                current.set_automation_id(next_automation_id);
            }
            if current.z_index() != z_index {
                current.set_z_index(z_index);
                paint_changed = true;
            }
        }

        tree.register_app_state_snapshot(id);

        let _handlers_changed = Self::reconcile_handlers(tree, id, handlers);
        tree.replace_render_handlers(id, render_handlers);
        let table_expand = tree.has_table_expand_renderer(id);
        let mut children_changed = if table_expand {
            let expanded = tree.table_expand_view(id).into_iter().collect();
            let changed = Self::reconcile_children(tree, id, expanded);
            tree.mark_table_expand_materialized(id);
            changed
        } else {
            Self::reconcile_children(tree, id, children)
        };
        children_changed |= tree.refresh_virtual_scroll_component(id, None);
        if children_changed {
            paint_changed = true;
            layout_changed = true;
        }

        if paint_changed {
            tree.invalidate_paint(id);
        }
        if layout_changed {
            tree.push_layout_invalidation(id);
            tree.propagate_layout_invalidation(id);
        }
    }

    fn reconcile_handlers(
        tree: &mut WidgetTree,
        id: ComponentId,
        handlers: Vec<HandlerRegistration>,
    ) -> bool {
        let next_signatures = tree
            .get(id)
            .map(|current| {
                Self::resolve_handler_signatures(current.handler_signatures(), &handlers)
            })
            .unwrap_or_else(|| {
                handlers
                    .iter()
                    .map(|handler| handler.authored_signature())
                    .collect()
            });
        let changed = tree.get(id).is_none_or(|current| {
            !Self::handler_signatures_are_stable(current.handler_signatures(), &next_signatures)
                || Self::handler_signature_groups(current.handler_signatures())
                    != Self::handler_signature_groups(&next_signatures)
        });
        if !changed {
            return false;
        }

        tree.handler_table().clear_component(id);
        if let Some(current) = tree.get_mut(id) {
            current.set_handler_signatures(next_signatures);
        }
        for handler in handlers {
            tree.handler_table().register(id, handler);
        }
        true
    }

    fn resolve_handler_signatures(
        current: &[HandlerSignature],
        handlers: &[HandlerRegistration],
    ) -> Vec<HandlerSignature> {
        let mut seen_by_kind = HashMap::new();
        handlers
            .iter()
            .map(|handler| {
                let mut next = handler.signature();
                if next.generation.is_some() || next.capture_fingerprint.is_none() {
                    return next;
                }

                let occurrence = seen_by_kind.entry(next.kind).or_insert(0);
                let current_signature =
                    Self::nth_handler_signature(current, next.kind, *occurrence);
                *occurrence += 1;

                next.generation = Some(match current_signature {
                    Some(current)
                        if current.capture_fingerprint == next.capture_fingerprint
                            && current.generation.is_some() =>
                    {
                        current.generation.unwrap_or(0)
                    }
                    Some(current) => current.generation.unwrap_or(0).saturating_add(1),
                    None => 0,
                });
                next
            })
            .collect()
    }

    fn nth_handler_signature(
        signatures: &[HandlerSignature],
        kind: SemanticKind,
        occurrence: usize,
    ) -> Option<&HandlerSignature> {
        signatures
            .iter()
            .filter(|signature| signature.kind == kind)
            .nth(occurrence)
    }

    fn handler_signature_groups(
        signatures: &[HandlerSignature],
    ) -> HashMap<SemanticKind, Vec<(Option<u32>, crate::ui::event::HandlerOptionsSignature)>> {
        let mut groups = HashMap::new();
        for signature in signatures {
            groups
                .entry(signature.kind)
                .or_insert_with(Vec::new)
                .push((signature.generation, signature.options));
        }
        groups
    }

    fn handler_signatures_are_stable(
        current: &[HandlerSignature],
        next: &[HandlerSignature],
    ) -> bool {
        current
            .iter()
            .all(|signature| signature.generation.is_some())
            && next.iter().all(|signature| signature.generation.is_some())
    }

    fn patch_widget(
        tree: &mut WidgetTree,
        id: ComponentId,
        widget: Box<dyn WidgetComponent>,
    ) -> bool {
        let Some(current) = tree.get_mut(id) else {
            return false;
        };

        let runtime_changed = builtin_widget_runtime_changed(current.component(), widget.as_ref());
        let next_fields = widget.snapshot_fields();
        let config_changed = current.component().snapshot_fields() != next_fields
            || next_fields == SnapshotFields::Unknown
            || runtime_changed;
        match patch_builtin_widget(current.component_mut(), widget) {
            Ok(true) => config_changed,
            Err(widget) => {
                current.replace_component(widget);
                config_changed
            }
            Ok(false) => false,
        }
    }

    fn reconcile_children(
        tree: &mut WidgetTree,
        parent_id: ComponentId,
        children: Vec<ViewNode>,
    ) -> bool {
        let old_children = tree
            .get(parent_id)
            .map(|node| node.children().to_vec())
            .unwrap_or_default();
        let mut old_by_key: HashMap<String, ComponentId> = HashMap::new();
        for &child_id in &old_children {
            if let Some(key) = tree.get(child_id).and_then(|node| node.key()) {
                old_by_key.insert(key.to_string(), child_id);
            }
        }

        let mut used_old = HashSet::new();
        let mut new_order = Vec::with_capacity(children.len());
        let mut structure_changed = old_children.len() != children.len();

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
                    structure_changed = true;
                    tree.build_child_node(parent_id, Self::expand(child))
                }
            } else {
                structure_changed = true;
                tree.build_child_node(parent_id, Self::expand(child))
            };
            new_order.push(child_id);
        }

        for child_id in old_children {
            if !used_old.contains(&child_id) && tree.get(child_id).is_some() {
                structure_changed = true;
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
            structure_changed = true;
        }
        structure_changed
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
