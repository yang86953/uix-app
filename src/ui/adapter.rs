//! ViewAdapter - expands a View tree into a WidgetTree.
//!
//! `App::run()` uses this module to recursively expand user-authored `View`
//! trees into framework `WidgetTree` nodes, keeping `WidgetNode` and
//! `BoxedWidget` internal.
//!
//! # Responsibilities
//!
//! 1. `ViewAdapter::build(root)` captures view context and builds a `WidgetTree`.
//! 2. `expand(node)` iteratively converts `ViewNode` into `WidgetNode`.
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
use crate::ui::accessibility::accessibility_override::AccessibilityOverride;
use crate::ui::animation::{begin_animated_capture, end_animated_capture};
use crate::ui::component::focus_handle::FocusHandle;
use crate::ui::component::traits::WidgetComponent;
use crate::ui::component::widget::{WidgetCore, WidgetNode};
use crate::ui::component_patch::{
    builtin_widget_config_changed, builtin_widget_runtime_changed, patch_builtin_widget,
};
use crate::ui::component_snapshot::SnapshotFields;
use crate::ui::event::system_event_handler::SystemEventHandlerRegistration;
use crate::ui::event::{HandlerRegistration, HandlerSignature, SemanticKind};
use crate::ui::reactive::state::{begin_state_capture, end_state_capture};
use crate::ui::render_handler::RenderHandlerRegistration;
use crate::ui::theme::style::Style;
#[cfg(any(test, feature = "test-harness"))]
use crate::ui::view::View;
use crate::ui::view::ViewNode;
use crate::ui::widgets::window_chrome::WindowInteractionRegion;
use crate::ui::widgets::{Button, Calendar, Container, Grid, Label};
use crate::ui::{ComponentId, WidgetTree};
use std::collections::{HashMap, HashSet};

/// 声明期 View 子节点能力端口（System 私有边界）。
///
/// `component → view` 依赖环消除（SMC-04）：`build_view_children` 从
/// `WidgetComponent` 移出，由本边界 trait 承载；树构建经 `as_view_children`
/// 上转型消费，widgets 经 `component!` 宏实现。
pub trait ViewChildrenProvider: WidgetComponent {
    fn build_view_children(&self) -> Vec<ViewNode>;
}

/// 读取组件的声明期 View 子节点（无端口时为空）。
pub(crate) fn view_children(widget: &dyn WidgetComponent) -> Vec<ViewNode> {
    widget
        .as_view_children()
        .map(|provider| provider.build_view_children())
        .unwrap_or_default()
}

/// View tree adapter.
pub struct ViewAdapter;

impl ViewAdapter {
    fn stagger_deadline(
        anchor: std::time::Instant,
        interval_secs: f64,
        rank: usize,
    ) -> Option<std::time::Instant> {
        if rank == 0 || interval_secs <= 0.0 {
            return None;
        }
        let delay = std::time::Duration::try_from_secs_f64(interval_secs * rank as f64)
            .unwrap_or(std::time::Duration::MAX);
        anchor.checked_add(delay).or_else(|| {
            let mut candidate = delay;
            loop {
                candidate /= 2;
                if let Some(deadline) = anchor.checked_add(candidate) {
                    break Some(deadline);
                }
            }
        })
    }

    fn configure_staggered_child(
        child: &mut ViewNode,
        stagger: Option<(f64, crate::ui::animation::AnimationConfig)>,
        anchor: std::time::Instant,
        rank: usize,
    ) {
        let Some((interval_secs, animation)) = stagger else {
            return;
        };
        child.enter_animation = Some(animation);
        child.enter_deadline = Self::stagger_deadline(anchor, interval_secs, rank);
    }

    /// Builds a View while capturing State bindings.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn capture_view(view: impl View) -> ViewNode {
        begin_state_capture();
        begin_animated_capture();
        let mut node = view.build();
        node.animated_sources = end_animated_capture();
        end_state_capture();
        node
    }

    /// Builds a ViewNode while capturing State bindings.
    pub fn capture_root<F>(build_root: F) -> ViewNode
    where
        F: FnOnce() -> ViewNode,
    {
        begin_state_capture();
        begin_animated_capture();
        let mut node = build_root();
        node.animated_sources = end_animated_capture();
        end_state_capture();
        node
    }

    /// Builds a View tree into a WidgetTree.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn build(view: impl View) -> WidgetTree {
        Self::build_nodes(Self::capture_view(view))
    }

    /// Builds an already expanded ViewNode tree into a WidgetTree.
    pub fn build_nodes(mut root: ViewNode) -> WidgetTree {
        let mut tree = WidgetTree::new();
        tree.sync_animated_sources(std::mem::take(&mut root.animated_sources));
        let wnode = Self::expand(root);
        tree.build(wnode);
        tree.bind_orphan_pending_states();
        tree.bind_pending_effects();
        tree
    }

    /// Reconciles a new View tree into an existing WidgetTree.
    #[cfg(any(test, feature = "test-harness"))]
    pub fn reconcile(tree: &mut WidgetTree, view: impl View) {
        Self::reconcile_nodes(tree, Self::capture_view(view));
    }

    /// Reconciles an already captured ViewNode tree into an existing WidgetTree.
    pub fn reconcile_nodes(tree: &mut WidgetTree, mut root: ViewNode) {
        tree.sync_animated_sources(std::mem::take(&mut root.animated_sources));
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

    /// Expands a ViewNode tree into a WidgetNode tree using explicit stack
    /// traversal to avoid stack overflow on deep trees in debug builds.
    pub(crate) fn expand(root: ViewNode) -> WidgetNode {
        // Decompose a ViewNode to keep traversal state on the heap.
        struct Frame {
            widget: Box<dyn WidgetComponent>,
            style: Style,
            visual_transform: crate::ui::component::view_transform::ViewTransform,
            enter_animation: Option<crate::ui::animation::AnimationConfig>,
            enter_deadline: Option<std::time::Instant>,
            leave_animation: Option<crate::ui::animation::AnimationConfig>,
            flex_grow_override: Option<f32>,
            flex_shrink_override: Option<f32>,
            provider_context: crate::ui::component::provider_context::ProviderContext,
            visible: bool,
            z_index: i32,
            key: Option<String>,
            automation_id: Option<String>,
            tab_index: Option<i32>,
            focus_handle: Option<FocusHandle>,
            accessibility_override: Option<AccessibilityOverride>,
            handlers: Vec<HandlerRegistration>,
            system_event_handlers: Vec<SystemEventHandlerRegistration>,
            render_handlers: Vec<RenderHandlerRegistration>,
            remaining_children: std::vec::IntoIter<ViewNode>,
            processed_children: Vec<WidgetNode>,
        }

        fn decompose(node: ViewNode) -> Frame {
            let ViewNode {
                widget,
                mut children,
                animated_sources: _,
                provider_context,
                style,
                visual_transform,
                enter_animation,
                enter_deadline,
                leave_animation,
                stagger_enter,
                flex_grow_override,
                flex_shrink_override,
                z_index,
                key,
                automation_id,
                tab_index,
                focus_handle,
                accessibility_override,
                handlers,
                system_event_handlers,
                render_handlers,
            } = node;
            let anchor = std::time::Instant::now();
            for (rank, child) in children.iter_mut().enumerate() {
                ViewAdapter::configure_staggered_child(child, stagger_enter, anchor, rank);
            }
            let visible = style.visible;
            Frame {
                widget,
                style,
                visual_transform,
                enter_animation,
                enter_deadline,
                leave_animation,
                flex_grow_override,
                flex_shrink_override,
                provider_context,
                visible,
                z_index,
                key,
                automation_id,
                tab_index,
                focus_handle,
                accessibility_override,
                handlers,
                system_event_handlers,
                render_handlers,
                remaining_children: children.into_iter(),
                processed_children: Vec::new(),
            }
        }

        fn build_widget(frame: Frame) -> WidgetNode {
            let widget = ViewAdapter::apply_style(
                frame.widget,
                &frame.style,
                frame.flex_grow_override,
                frame.flex_shrink_override,
            );

            let mut wnode = if frame.processed_children.is_empty() {
                WidgetNode::leaf(widget)
            } else {
                WidgetNode::new(widget, frame.processed_children)
            };

            if let Some(key) = frame.key {
                wnode = wnode.key(&key);
            }
            if let Some(automation_id) = frame.automation_id {
                wnode = wnode.automation_id(&automation_id);
            }
            if let Some(tab_index) = frame.tab_index {
                wnode = wnode.tab_index(tab_index);
            }
            if let Some(focus_handle) = frame.focus_handle {
                wnode = wnode.with_focus_handle(focus_handle);
            }
            if let Some(accessibility_override) = frame.accessibility_override {
                wnode = wnode.with_accessibility_override(accessibility_override);
            }
            if frame.z_index != 0 {
                wnode = wnode.z_index(frame.z_index);
            }
            if !frame.visible {
                wnode = wnode.with_visibility(false);
            }
            if frame.visual_transform
                != crate::ui::component::view_transform::ViewTransform::default()
            {
                wnode = wnode.with_visual_transform(frame.visual_transform);
            }
            if let Some(animation) = frame.enter_animation {
                wnode = wnode.with_enter_animation(animation, frame.enter_deadline);
            }
            if let Some(animation) = frame.leave_animation {
                wnode = wnode.with_leave_animation(animation);
            }
            if !frame.handlers.is_empty() {
                wnode = wnode.with_handlers(frame.handlers);
            }
            if !frame.system_event_handlers.is_empty() {
                wnode = wnode.with_system_event_handlers(frame.system_event_handlers);
            }
            if !frame.render_handlers.is_empty() {
                wnode = wnode.with_render_handlers(frame.render_handlers);
            }

            wnode.with_provider_context(frame.provider_context)
        }

        let mut stack: Vec<Frame> = Vec::new();
        let mut current = decompose(root);

        loop {
            if let Some(child) = current.remaining_children.next() {
                stack.push(current);
                current = decompose(child);
                continue;
            }

            let wnode = build_widget(current);
            match stack.pop() {
                Some(mut parent) => {
                    parent.processed_children.push(wnode);
                    current = parent;
                }
                None => return wnode,
            }
        }
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
                let mut button_style = style.clone();
                if let Some(g) = flex_grow_override {
                    button_style.flex_grow = g;
                }
                if let Some(s) = flex_shrink_override {
                    button_style.flex_shrink = s;
                }
                b.style = button_style.into();
            }
        } else if tid == std::any::TypeId::of::<Grid>() {
            if let Some(g) = widget.as_any_mut().downcast_mut::<Grid>() {
                g.apply_style(style);
            }
        } else if tid == std::any::TypeId::of::<crate::ui::component::dynamic_label::DynamicLabel>()
        {
            if let Some(dl) = widget
                .as_any_mut()
                .downcast_mut::<crate::ui::component::dynamic_label::DynamicLabel>()
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
            animated_sources: _,
            provider_context,
            style,
            visual_transform,
            enter_animation: _,
            enter_deadline: _,
            leave_animation,
            stagger_enter,
            flex_grow_override,
            flex_shrink_override,
            z_index,
            key,
            automation_id,
            tab_index,
            focus_handle,
            accessibility_override,
            handlers,
            system_event_handlers,
            render_handlers,
        } = node;
        let context_changed = tree
            .get(id)
            .is_none_or(|current| current.provider_context() != &provider_context);
        if let Some(current) = tree.get_mut(id) {
            current.set_provider_context(provider_context);
            current.set_leave_animation(leave_animation);
        }
        if !style.visible {
            tree.set_node_visibility(id, false);
        }
        let widget = Self::apply_style(widget, &style, flex_grow_override, flex_shrink_override);
        // Calendar cells depend on preserved runtime month/selection. Building them from
        // the freshly declared widget here would invoke the factory with stale defaults;
        // reconcile them after `sync_from` has patched the live Calendar instead.
        let calendar_cells = widget
            .as_any()
            .downcast_ref::<Calendar>()
            .is_some_and(Calendar::owns_custom_cell_children)
            || tree.is_calendar_cell_component(id);
        let component_view_children = if calendar_cells {
            Vec::new()
        } else {
            view_children(widget.as_ref())
        };
        let next_accessibility = widget.snapshot_fields().accessibility();
        let next_disabled = accessibility_override
            .as_ref()
            .map(|override_state| override_state.apply(next_accessibility.clone()))
            .unwrap_or(next_accessibility)
            .state
            .disabled;
        if next_disabled {
            // PointerLeave / DragEnd 必须在旧组件仍启用时交付，随后再 patch disabled。
            tree.cancel_pointer_hover_in_subtree(id);
            tree.cancel_pointer_gesture_in_subtree(id);
        }
        if next_disabled
            && tree
                .managers()
                .focus
                .focused_component()
                .is_some_and(|focused| tree.is_descendant_of(focused, id))
        {
            // 旧组件仍启用时先交付 FocusOut，清理键盘按压和控件视觉；
            // 随后的 patch 才写入 disabled，避免 disabled 早退吞掉清理事件。
            tree.set_focus(None);
        }
        let widget_changed = Self::patch_widget(tree, id, widget);
        if style.visible {
            tree.set_node_visibility(id, true);
        }
        tree.set_tab_index_override(id, tab_index);
        tree.set_focus_handle(id, focus_handle);
        if let Some(current) = tree.get_mut(id) {
            current.set_accessibility_override(accessibility_override);
        }

        let mut paint_changed = widget_changed || context_changed;
        let mut layout_changed = widget_changed || context_changed;
        if tree.set_visual_transform(id, visual_transform) {
            paint_changed = true;
        }
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
        tree.replace_system_event_handlers(id, system_event_handlers);
        tree.replace_render_handlers(id, render_handlers);
        // 表格 capability 启用时才协调泛型单元格或扩展行动态子树。
        #[cfg(feature = "table")]
        let table_children_changed = if tree.has_table_cell_renderer(id) {
            Some(tree.refresh_table_cell_component(id))
        } else if tree.has_table_expand_renderer(id) {
            let expanded = tree.table_expand_view(id).into_iter().collect();
            let changed = Self::reconcile_children(tree, id, expanded, None);
            tree.mark_table_expand_materialized(id);
            Some(changed)
        } else {
            None
        };
        // 表格 capability 关闭时不保留专属动态子树协调结果。
        #[cfg(not(feature = "table"))]
        let table_children_changed = None::<bool>;
        let select_options = tree.has_select_option_renderer(id);
        // VirtualScroll 的物化行由专用 keyed 动态协调器拥有。
        let virtual_scroll_items = tree.has_virtual_scroll_renderer(id);
        let collapse_content = tree.is_collapse_content_component(id);
        if select_options {
            tree.invalidate_select_option_component(id);
        }
        let mut children_changed = if let Some(changed) = table_children_changed {
            changed
        } else if select_options {
            tree.refresh_select_option_component(id)
        } else if collapse_content {
            tree.refresh_collapse_content_component(id)
        } else if calendar_cells {
            tree.refresh_calendar_cell_component(id)
        } else if virtual_scroll_items {
            // 保留旧物化窗口，随后用新版 renderer 按稳定 key 原位协调。
            false
        } else {
            let mut children = children;
            children.extend(component_view_children);
            Self::reconcile_children(tree, id, children, stagger_enter)
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
        let config_changed = builtin_widget_config_changed(current.component(), widget.as_ref())
            .unwrap_or_else(|| current.component().snapshot_fields() != next_fields)
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
        stagger_enter: Option<(f64, crate::ui::animation::AnimationConfig)>,
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
        let stagger_anchor = std::time::Instant::now();
        let mut mounted_rank = 0;

        for (index, mut child) in children.into_iter().enumerate() {
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
                    tree.cancel_pending_removal(child_id);
                    Self::reconcile_existing(tree, child_id, child);
                    child_id
                } else {
                    tree.remove(child_id);
                    structure_changed = true;
                    Self::configure_staggered_child(
                        &mut child,
                        stagger_enter,
                        stagger_anchor,
                        mounted_rank,
                    );
                    mounted_rank += 1;
                    tree.build_child_node(parent_id, Self::expand(child))
                }
            } else {
                structure_changed = true;
                Self::configure_staggered_child(
                    &mut child,
                    stagger_enter,
                    stagger_anchor,
                    mounted_rank,
                );
                mounted_rank += 1;
                tree.build_child_node(parent_id, Self::expand(child))
            };
            new_order.push(child_id);
        }

        for (old_index, child_id) in old_children.iter().copied().enumerate() {
            if !used_old.contains(&child_id) && tree.get(child_id).is_some() {
                structure_changed = true;
                if tree.start_leave_transition(child_id) {
                    new_order.insert(old_index.min(new_order.len()), child_id);
                } else {
                    tree.remove(child_id);
                }
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

    pub(crate) fn reconcile_dynamic_children(
        tree: &mut WidgetTree,
        parent_id: ComponentId,
        children: Vec<ViewNode>,
    ) -> bool {
        Self::reconcile_children(tree, parent_id, children, None)
    }
}
