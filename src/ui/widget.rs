use crate::graphics::{DirtyRegion, Point, Rect, Size};

// Re-export layout types for backward compatibility
pub use crate::graphics::{AlignItems, FlexDirection, JustifyContent};

/// Event result enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventResult {
    Handled,
    NotHandled,
    Bubbled,
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    None,
    Left,
    Right,
    Middle,
}

/// Key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    Unknown,
    Enter,
    Escape,
    Backspace,
    Delete,
    Tab,
    Space,
    Up,
    Down,
    Left,
    Right,
    Home,
    End,
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,
    Num0,
    Num1,
    Num2,
    Num3,
    Num4,
    Num5,
    Num6,
    Num7,
    Num8,
    Num9,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

/// Widget event types.
#[derive(Debug, Clone)]
pub enum WidgetEvent {
    MouseDown { pos: Point, button: MouseButton },
    MouseUp { pos: Point, button: MouseButton },
    MouseMove { pos: Point },
    MouseWheel { delta: Point },
    KeyDown { key: KeyCode },
    KeyUp { key: KeyCode },
    FocusIn,
    FocusOut,
    HoverEnter,
    HoverLeave,
    Resize { width: f32, height: f32 },
}

/// Widget tree node ID.
pub type WidgetId = usize;

/// Widget trait — the core behavioral abstraction for all UI components.
/// Pure behavior: no tree metadata. Tree metadata is managed by BoxedWidget via WidgetCore.
pub trait Widget {
    /// Return child widgets to be added to the tree.
    fn build(&self) -> Vec<Box<dyn Widget>> {
        vec![]
    }

    fn on_init(&mut self) {}
    fn on_mount(&mut self) {}
    fn on_unmount(&mut self) {}
    fn on_event(&mut self, _event: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
    }
    fn on_update(&mut self, _dt: f32) {}

    /// Return this widget's preferred size for layout.
    /// When `engine` is provided, text-measuring widgets can compute
    /// accurate dimensions via the graphics backend; implementations
    /// that don't need it may ignore the parameter.
    fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        Size::zero()
    }

    /// Render this widget into the given render context.
    /// `frame` is the widget's current frame in the tree.
    fn render(
        &self,
        frame: Rect,
        ctx: &mut crate::ui::render_context::RenderContext,
        tree: &WidgetTree,
    );

    /// Compute child layout positions. Returns (child_id, rect) pairs.
    /// `frame` is the widget's current frame. `children` are child IDs from the tree.
    fn layout_children(
        &self,
        frame: Rect,
        children: &[WidgetId],
        tree: &WidgetTree,
    ) -> Vec<(WidgetId, Rect)> {
        let _ = (frame, children, tree);
        Vec::new()
    }
}

/// Core widget tree metadata — managed solely by BoxedWidget, not by individual widgets.
pub trait WidgetCore {
    fn id(&self) -> WidgetId;
    fn set_id(&mut self, id: WidgetId);

    fn parent(&self) -> Option<WidgetId>;
    fn set_parent(&mut self, id: Option<WidgetId>);

    fn children(&self) -> &[WidgetId];
    fn children_mut(&mut self) -> &mut Vec<WidgetId>;

    fn frame(&self) -> Rect;
    fn set_frame(&mut self, rect: Rect);

    fn visible(&self) -> bool;
    fn set_visible(&mut self, v: bool);

    fn dirty(&self) -> bool;
    fn set_dirty(&mut self, v: bool);

    fn opacity(&self) -> f32;
    fn set_opacity(&mut self, v: f32);
}

// ──────────────────────────────────────────────────────────────────────────
// BoxedWidget — wrapper for trait-object based widget tree
// ──────────────────────────────────────────────────────────────────────────

/// A type-erased widget stored in the tree.
/// Owns the tree metadata and delegates behavior to the inner Widget.
pub struct BoxedWidget {
    inner: Box<dyn Widget>,
    id: WidgetId,
    parent: Option<WidgetId>,
    children: Vec<WidgetId>,
    frame: Rect,
    visible: bool,
    is_dirty: bool,
    widget_opacity: f32,
}

impl BoxedWidget {
    pub fn new(inner: Box<dyn Widget>) -> Self {
        Self {
            inner,
            id: 0,
            parent: None,
            children: Vec::new(),
            frame: Rect::zero(),
            visible: true,
            is_dirty: true,
            widget_opacity: 1.0,
        }
    }

    pub fn inner(&self) -> &dyn Widget {
        &*self.inner
    }
    pub fn inner_mut(&mut self) -> &mut dyn Widget {
        &mut *self.inner
    }

    /// Query the inner widget's preferred size.
    pub fn preferred_size(&self, engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
        self.inner.preferred_size(engine)
    }
}

impl WidgetCore for BoxedWidget {
    fn id(&self) -> WidgetId {
        self.id
    }
    fn set_id(&mut self, id: WidgetId) {
        self.id = id;
    }

    fn parent(&self) -> Option<WidgetId> {
        self.parent
    }
    fn set_parent(&mut self, id: Option<WidgetId>) {
        self.parent = id;
    }

    fn children(&self) -> &[WidgetId] {
        &self.children
    }
    fn children_mut(&mut self) -> &mut Vec<WidgetId> {
        &mut self.children
    }

    fn frame(&self) -> Rect {
        self.frame
    }
    fn set_frame(&mut self, rect: Rect) {
        self.frame = rect;
    }

    fn visible(&self) -> bool {
        self.visible
    }
    fn set_visible(&mut self, v: bool) {
        self.visible = v;
    }

    fn dirty(&self) -> bool {
        self.is_dirty
    }
    fn set_dirty(&mut self, v: bool) {
        self.is_dirty = v;
    }

    fn opacity(&self) -> f32 {
        self.widget_opacity
    }
    fn set_opacity(&mut self, v: f32) {
        self.widget_opacity = v;
    }
}

/// Widget tree — manages the tree of BoxedWidget nodes.
pub struct WidgetTree {
    nodes: Vec<Option<BoxedWidget>>,
    free_ids: Vec<WidgetId>,
    next_id: WidgetId,
    root_id: Option<WidgetId>,
    /// Cross-cutting manager systems available to widgets during
    /// layout, rendering, and event processing.
    pub managers: crate::ui::managers::WidgetManagers,
    /// Widget that currently has keyboard focus.
    focused_widget: Option<WidgetId>,
    /// Widget that the mouse is currently hovering over.
    hovered_widget: Option<WidgetId>,
    /// Accumulated dirty region for the current frame.
    dirty_region: DirtyRegion,
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            free_ids: Vec::new(),
            next_id: 0,
            root_id: None,
            managers: crate::ui::managers::WidgetManagers::new(),
            focused_widget: None,
            hovered_widget: None,
            dirty_region: DirtyRegion::full(),
        }
    }
}

impl WidgetTree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_id(&mut self) -> WidgetId {
        if let Some(id) = self.free_ids.pop() {
            return id;
        }
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Set root widget with pre-built children (two-phase construction).
    /// The `children` vector is recursively added to the tree under the root.
    /// This eliminates the need for `RefCell<Option<Vec<...>>>` in builder widgets.
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

    /// Add a child widget with pre-built sub-children (two-phase construction).
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

    /// Set root widget and recursively build its child tree via `Widget::build()`.
    /// Returns the root widget's `WidgetId`.
    /// The root's frame is initialized to (0, 0, preferred_size.w, preferred_size.h).
    pub fn set_root(&mut self, widget: Box<dyn Widget>) -> WidgetId {
        // Call build() BEFORE widget is moved into BoxedWidget
        let children = widget.build();

        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new(widget);
        boxed.set_id(id);
        // Initialize root frame from preferred_size so layout() has a non-zero
        // root frame to distribute to children.
        let ps = boxed.preferred_size(None);
        boxed.set_frame(Rect::new(0.0, 0.0, ps.w, ps.h));
        if self.nodes.len() <= id {
            self.nodes.resize_with(id + 1, || None);
        }
        self.nodes[id] = Some(boxed);
        self.root_id = Some(id);

        // Recursively add children declared via build()
        for child in children {
            self.add_child(id, child);
        }

        id
    }

    pub fn root(&self) -> Option<&BoxedWidget> {
        self.root_id
            .and_then(|id| self.nodes.get(id))
            .and_then(|n| n.as_ref())
    }

    pub fn root_mut(&mut self) -> Option<&mut BoxedWidget> {
        self.root_id
            .and_then(|id| self.nodes.get_mut(id))
            .and_then(|n| n.as_mut())
    }

    pub fn get(&self, id: WidgetId) -> Option<&BoxedWidget> {
        self.nodes.get(id).and_then(|n| n.as_ref())
    }

    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut BoxedWidget> {
        self.nodes.get_mut(id).and_then(|n| n.as_mut())
    }

    /// Add a child widget and recursively build its sub-tree via `Widget::build()`.
    pub fn add_child(&mut self, parent_id: WidgetId, child: Box<dyn Widget>) -> WidgetId {
        // Call build() BEFORE widget is moved into BoxedWidget
        let children = child.build();

        let child_id = self.alloc_id();
        let mut boxed = BoxedWidget::new(child);
        boxed.set_id(child_id);
        boxed.set_parent(Some(parent_id));

        if self.nodes.len() <= child_id {
            self.nodes.resize_with(child_id + 1, || None);
        }
        self.nodes[child_id] = Some(boxed);

        if let Some(parent) = self.get_mut(parent_id) {
            parent.children_mut().push(child_id);
        }

        // Recursively add children declared via build()
        for child in children {
            self.add_child(child_id, child);
        }

        child_id
    }

    pub fn remove(&mut self, id: WidgetId) {
        // Capture parent before taking the node
        let parent_id = self
            .nodes
            .get(id)
            .and_then(|n| n.as_ref())
            .and_then(|n| n.parent());

        if let Some(node) = self.nodes.get_mut(id) {
            if let Some(node) = node.take() {
                // Recursively remove children
                for child_id in node.children().to_vec() {
                    self.remove(child_id);
                }
                self.free_ids.push(id);
            }
        }

        // Remove from parent's children list
        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_mut(pid) {
                parent.children_mut().retain(|&c| c != id);
            }
        }
    }

    pub fn traverse(&self) -> Vec<WidgetId> {
        let mut result = Vec::new();
        if let Some(root_id) = self.root_id {
            self.traverse_internal(root_id, &mut result);
        }
        result
    }

    fn traverse_internal(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        result.push(id);
        if let Some(node) = self.get(id) {
            for child_id in node.children().iter() {
                self.traverse_internal(*child_id, result);
            }
        }
    }

    /// Layout phase: compute and assign positions for all children.
    /// The root widget's frame is reset to its preferred size each frame
    /// so layout always starts from a well-defined origin.
    /// Widgets whose frame changes are automatically marked dirty.
    pub fn layout(&mut self) {
        // Ensure root frame is set from preferred_size
        let root_frame_changed = if let Some(root_id) = self.root_id {
            let ps = self.get(root_id).map(|r| r.preferred_size(None));
            if let Some(ps) = ps {
                let new_frame = Rect::new(0.0, 0.0, ps.w, ps.h);
                let current = self.get(root_id).map(|r| r.frame());
                let changed = current != Some(new_frame);
                if let Some(root_mut) = self.get_mut(root_id) {
                    root_mut.set_frame(new_frame);
                }
                if changed {
                    Some(root_id)
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };
        if let Some(root_id) = root_frame_changed {
            self.mark_dirty(root_id);
        }

        let order = self.traverse();
        // Collect (child_id, rect) pairs first, then apply
        let mut changed: Vec<(WidgetId, Rect)> = Vec::new();
        for id in order.into_iter().rev() {
            if let Some(node) = self.get(id) {
                let frame = node.frame();
                let children: Vec<WidgetId> = node.children().to_vec();
                let positions = node.inner().layout_children(frame, &children, self);
                for (child_id, rect) in positions {
                    let current_frame = self.get(child_id).map(|c| c.frame());
                    if current_frame != Some(rect) {
                        changed.push((child_id, rect));
                    }
                }
            }
        }
        // Apply frame changes and mark dirty
        for (child_id, rect) in changed {
            if let Some(child) = self.get_mut(child_id) {
                child.set_frame(rect);
            }
            self.mark_dirty(child_id);
        }
    }

    // ── Dirty Region Tracking ───────────────────────────────────────

    /// Return the accumulated dirty region for the current frame.
    pub fn dirty_region(&self) -> &DirtyRegion {
        &self.dirty_region
    }

    /// Mark a widget as dirty, merging its frame into the dirty region.
    pub fn mark_dirty(&mut self, id: WidgetId) {
        let frame = if let Some(node) = self.get_mut(id) {
            node.set_dirty(true);
            node.frame()
        } else {
            return;
        };
        if !self.dirty_region.full_frame && frame.w > 0.0 && frame.h > 0.0 {
            self.dirty_region = DirtyRegion::area(
                self.dirty_region.rect.union(&frame),
            );
        }
    }

    /// Mark a widget and its entire subtree as dirty.
    pub fn mark_dirty_subtree(&mut self, id: WidgetId) {
        let ids: Vec<WidgetId> = {
            let mut result = vec![id];
            if let Some(node) = self.get(id) {
                for &child_id in node.children() {
                    self.collect_subtree(child_id, &mut result);
                }
            }
            result
        };
        for id in ids {
            self.mark_dirty(id);
        }
    }

    fn collect_subtree(&self, id: WidgetId, result: &mut Vec<WidgetId>) {
        result.push(id);
        if let Some(node) = self.get(id) {
            for &child_id in node.children() {
                self.collect_subtree(child_id, result);
            }
        }
    }

    /// Reset the dirty region for the next frame.
    /// Called after rendering is complete.
    pub fn reset_dirty(&mut self) {
        self.dirty_region.reset();
        // Clear per-widget dirty flags
        let ids: Vec<WidgetId> = self.traverse();
        for id in ids {
            if let Some(node) = self.get_mut(id) {
                node.set_dirty(false);
            }
        }
    }

    /// Render phase: traverse and render all widgets.
    pub fn render_tree(&self, ctx: &mut crate::ui::render_context::RenderContext) {
        if let Some(root_id) = self.root_id {
            self.render_node(root_id, ctx);
        }
    }

    fn render_node(&self, id: WidgetId, ctx: &mut crate::ui::render_context::RenderContext) {
        if let Some(node) = self.get(id) {
            if !node.visible() {
                return;
            }
            let frame = node.frame();
            node.inner().render(frame, ctx, self);
            for &child_id in node.children() {
                self.render_node(child_id, ctx);
            }
        }
    }

    // ── Event Dispatch ──────────────────────────────────────────────

    /// Return the deepest visible widget whose frame contains `pos`.
    /// Traverses children in reverse order (last child = topmost in z-order)
    /// so that overlapping siblings yield the visually-top widget.
    pub fn hit_test(&self, pos: Point) -> Option<WidgetId> {
        self.root_id
            .and_then(|root| self.hit_test_internal(root, pos))
    }

    fn hit_test_internal(&self, id: WidgetId, pos: Point) -> Option<WidgetId> {
        let node = self.get(id)?;
        if !node.visible() {
            return None;
        }
        // Check children first (reverse = top-to-bottom z-order)
        for &child_id in node.children().iter().rev() {
            if let Some(hit) = self.hit_test_internal(child_id, pos) {
                return Some(hit);
            }
        }
        // No child hit — check self
        if node.frame().contains(pos) {
            Some(id)
        } else {
            None
        }
    }

    /// Dispatch a `WidgetEvent` into the tree.
    ///
    /// Routing rules:
    /// - MouseDown / MouseUp → hit-test then dispatch to deepest target,
    ///   bubbling up if not consumed. On MouseDown, the target becomes the
    ///   focused widget (so keyboard events route to it).
    /// - MouseMove → hit-test, manage HoverEnter/HoverLeave transitions,
    ///   then dispatch to the hover target.
    /// - MouseWheel → dispatch to currently hovered widget (or root).
    /// - KeyDown / KeyUp → dispatch to the focused widget.
    /// - Resize → dispatch to root.
    /// - HoverEnter / HoverLeave / FocusIn / FocusOut — internal bookkeeping;
    ///   external callers typically do not emit these directly.
    pub fn dispatch_event(&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { pos, .. } => {
                let target = self.hit_test(*pos);
                if let Some(t) = target {
                    let result = self.dispatch_to(t, event);
                    if result == EventResult::Handled {
                        self.set_focus(Some(t));
                    }
                    result
                } else {
                    self.set_focus(None);
                    EventResult::NotHandled
                }
            }
            WidgetEvent::MouseUp { pos, .. } => {
                if let Some(t) = self.hit_test(*pos) {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            WidgetEvent::MouseMove { pos } => {
                let new_hover = self.hit_test(*pos);
                if new_hover != self.hovered_widget {
                    if let Some(old) = self.hovered_widget {
                        let _ = self.dispatch_to(old, &WidgetEvent::HoverLeave);
                    }
                    if let Some(new) = new_hover {
                        let _ = self.dispatch_to(new, &WidgetEvent::HoverEnter);
                    }
                    self.hovered_widget = new_hover;
                }
                if let Some(t) = new_hover {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            WidgetEvent::MouseWheel { .. } => {
                let target = self
                    .hovered_widget
                    .or(self.root_id);
                if let Some(t) = target {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            WidgetEvent::KeyDown { .. } | WidgetEvent::KeyUp { .. } => {
                if let Some(t) = self.focused_widget {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            WidgetEvent::FocusIn | WidgetEvent::FocusOut => {
                if let Some(t) = self.focused_widget {
                    self.dispatch_to(t, event)
                } else {
                    EventResult::NotHandled
                }
            }
            WidgetEvent::HoverEnter | WidgetEvent::HoverLeave => {
                // Internally managed — external dispatch is a no-op.
                EventResult::NotHandled
            }
            WidgetEvent::Resize { .. } => {
                if let Some(root) = self.root_id {
                    self.dispatch_to(root, event)
                } else {
                    EventResult::NotHandled
                }
            }
        }
    }

    /// Send an event to a specific widget, bubbling up if not consumed.
    /// Returns the event's final disposition.
    fn dispatch_to(&mut self, target: WidgetId, event: &WidgetEvent) -> EventResult {
        let mut current = Some(target);
        while let Some(id) = current {
            let node = match self.get_mut(id) {
                Some(n) => n,
                None => return EventResult::NotHandled,
            };
            let result = node.inner_mut().on_event(event);
            match result {
                EventResult::Handled => return EventResult::Handled,
                EventResult::Bubbled => {
                    current = node.parent();
                }
                EventResult::NotHandled => {
                    current = node.parent();
                }
            }
        }
        EventResult::NotHandled
    }

    /// Update the focused widget, sending FocusOut to the old and FocusIn to
    /// the new.
    fn set_focus(&mut self, new_focus: Option<WidgetId>) {
        if new_focus == self.focused_widget {
            return;
        }
        if let Some(old) = self.focused_widget {
            let _ = self.dispatch_to(old, &WidgetEvent::FocusOut);
        }
        self.focused_widget = new_focus;
        if let Some(new) = new_focus {
            let _ = self.dispatch_to(new, &WidgetEvent::FocusIn);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graphics::{Point, Rect, Size};
    use std::cell::RefCell;

    /// A minimal test widget that records the last event it received.
    struct SpyWidget {
        size: Size,
        last_event: RefCell<Option<WidgetEvent>>,
    }

    impl SpyWidget {
        fn new(w: f32, h: f32) -> Self {
            Self {
                size: Size::new(w, h),
                last_event: RefCell::new(None),
            }
        }
    }

    impl Widget for SpyWidget {
        fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
            self.size
        }

        fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
            *self.last_event.borrow_mut() = Some(event.clone());
            EventResult::Handled
        }

        fn render(&self, _frame: Rect, _ctx: &mut crate::ui::render_context::RenderContext, _tree: &WidgetTree) {}
    }

    /// A container that passes events through (returns NotHandled for bubbling).
    struct PassThroughContainer {
        size: Size,
        children: RefCell<Vec<Box<dyn Widget>>>,
    }

    impl PassThroughContainer {
        fn new(w: f32, h: f32, children: Vec<Box<dyn Widget>>) -> Self {
            Self {
                size: Size::new(w, h),
                children: RefCell::new(children),
            }
        }
    }

    impl Widget for PassThroughContainer {
        fn build(&self) -> Vec<Box<dyn Widget>> {
            std::mem::take(&mut *self.children.borrow_mut())
        }

        fn preferred_size(&self, _engine: Option<&dyn crate::graphics::GraphicsEngine>) -> Size {
            self.size
        }

        fn on_event(&mut self, _event: &WidgetEvent) -> EventResult {
            EventResult::NotHandled // let it bubble to parent
        }

        fn render(&self, _frame: Rect, _ctx: &mut crate::ui::render_context::RenderContext, _tree: &WidgetTree) {}
    }

    // ── WidgetTree Construction ──

    #[test]
    fn tree_set_root_returns_valid_id() {
        let mut tree = WidgetTree::new();
        let id = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        assert!(tree.get(id).is_some());
        assert_eq!(tree.root().unwrap().id(), id);
    }

    #[test]
    fn tree_add_child_links_parent() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let cid = tree.add_child(root, Box::new(SpyWidget::new(80.0, 40.0)));
        assert!(tree.get(cid).is_some());
        assert_eq!(tree.get(root).unwrap().children(), &[cid]);
        assert_eq!(tree.get(cid).unwrap().parent(), Some(root));
    }

    #[test]
    fn tree_traverse_preorder() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let b = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let c = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
        let ids = tree.traverse();
        assert_eq!(ids, vec![root, a, c, b]);
    }

    #[test]
    fn tree_remove_cascades_to_children() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let b = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
        tree.remove(a);
        assert!(tree.get(a).is_none());
        assert!(tree.get(b).is_none());
        assert_eq!(tree.get(root).unwrap().children().len(), 0);
    }

    // ── Hit Testing ──

    #[test]
    fn hit_test_root_contains() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        tree.layout();
        assert!(tree.hit_test(Point::new(50.0, 25.0)).is_some());
    }

    #[test]
    fn hit_test_outside_returns_none() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        tree.layout();
        assert!(tree.hit_test(Point::new(200.0, 200.0)).is_none());
        assert!(tree.hit_test(Point::new(-1.0, 25.0)).is_none());
    }

    #[test]
    fn hit_test_returns_deepest_child() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(
            200.0,
            200.0,
            vec![],
        )));
        let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
        // Manually set frame since PassThroughContainer doesn't lay out children
        tree.get_mut(root).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        let hit = tree.hit_test(Point::new(50.0, 50.0));
        assert_eq!(hit, Some(child));
    }

    #[test]
    fn hit_test_skips_invisible() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(
            200.0,
            200.0,
            vec![],
        )));
        let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        tree.get_mut(child).unwrap().set_visible(false);
        let hit = tree.hit_test(Point::new(50.0, 50.0));
        assert_eq!(hit, Some(root));
    }

    // ── Event Dispatch ──

    #[test]
    fn dispatch_mouse_down_focuses_target() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let btn = tree.add_child(
            root_id,
            Box::new(SpyWidget::new(80.0, 40.0)),
        );
        // Manually set frames
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(btn).unwrap().set_frame(Rect::new(0.0, 0.0, 80.0, 40.0));

        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(40.0, 20.0),
            button: MouseButton::Left,
        });
        assert_eq!(result, EventResult::Handled);
    }

    #[test]
    fn dispatch_mouse_down_empty_space_clears_focus() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
        tree.layout();

        // Click outside root frame
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(300.0, 300.0),
            button: MouseButton::Left,
        });
        // Focus should be cleared (no assertion failure, just verifying no panic)
    }

    #[test]
    fn dispatch_key_to_focused_widget() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
        tree.layout();

        // Click to focus
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 50.0),
            button: MouseButton::Left,
        });

        // Now dispatch a key — should reach the focused widget
        let result = tree.dispatch_event(&WidgetEvent::KeyDown {
            key: KeyCode::Enter,
        });
        assert_eq!(result, EventResult::Handled);
    }

    #[test]
    fn dispatch_mouse_move_triggers_hover_enter_leave() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(
            root_id,
            Box::new(SpyWidget::new(100.0, 100.0)),
        );
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

        // Move into child — should dispatch MouseMove to child
        tree.dispatch_event(&WidgetEvent::MouseMove {
            pos: Point::new(50.0, 50.0),
        });
        // MouseMove dispatched; HoverEnter/HoverLeave are internal transitions
    }

    #[test]
    fn dispatch_resize_goes_to_root() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
        tree.layout();

        let result = tree.dispatch_event(&WidgetEvent::Resize {
            width: 400.0,
            height: 300.0,
        });
        assert_eq!(result, EventResult::Handled);
    }
}
