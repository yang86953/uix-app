use super::*;
use crate::base::Rect;
use crate::graphics::DirtyRegion;

/// Widget tree — 管理 BoxedWidget 节点树。
pub struct WidgetTree {
    pub(crate) nodes: Vec<Option<BoxedWidget>>,
    pub(crate) free_ids: Vec<WidgetId>,
    pub(crate) next_id: WidgetId,
    pub(crate) root_id: Option<WidgetId>,
    pub(crate) focused_widget: Option<WidgetId>,
    pub(crate) hovered_widget: Option<WidgetId>,
    pub(crate) dirty_region: DirtyRegion,
    pub(crate) mouse_down_target: Option<WidgetId>,
    pub(crate) scroll_deltas: Vec<(Rect, f32, f32)>,
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self {
            nodes: Vec::new(),
            free_ids: Vec::new(),
            next_id: 0,
            root_id: None,
            focused_widget: None,
            hovered_widget: None,
            dirty_region: DirtyRegion::full(),
            mouse_down_target: None,
            scroll_deltas: Vec::new(),
        }
    }
}

impl WidgetTree {
    pub fn new() -> Self { Self::default() }

    pub fn alloc_id(&mut self) -> WidgetId {
        if let Some(id) = self.free_ids.pop() { return id; }
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn set_root_with_children(
        &mut self, widget: Box<dyn Widget>, children: Vec<Box<dyn Widget>>,
    ) -> WidgetId {
        let id = self.set_root(widget);
        for child in children { self.add_child(id, child); }
        id
    }

    pub fn add_child_with_children(
        &mut self, parent_id: WidgetId, widget: Box<dyn Widget>, children: Vec<Box<dyn Widget>>,
    ) -> WidgetId {
        let id = self.add_child(parent_id, widget);
        for child in children { self.add_child(id, child); }
        id
    }

    pub fn set_root(&mut self, widget: Box<dyn Widget>) -> WidgetId {
        let children = widget.build();
        let id = self.alloc_id();
        let mut boxed = BoxedWidget::new(widget);
        boxed.set_id(id);
        let ps = boxed.preferred_size(None);
        boxed.set_frame(Rect::new(0.0, 0.0, ps.w, ps.h));
        if self.nodes.len() <= id {
            self.nodes.resize_with(id + 1, || None);
        }
        self.nodes[id] = Some(boxed);
        self.root_id = Some(id);
        for child in children { self.add_child(id, child); }
        id
    }

    pub fn root(&self) -> Option<&BoxedWidget> {
        self.root_id.and_then(|id| self.nodes.get(id)).and_then(|n| n.as_ref())
    }
    pub fn root_id(&self) -> Option<WidgetId> { self.root_id }
    pub fn root_mut(&mut self) -> Option<&mut BoxedWidget> {
        self.root_id.and_then(|id| self.nodes.get_mut(id)).and_then(|n| n.as_mut())
    }

    pub fn find_by_type<T: Widget + 'static>(&self) -> Option<WidgetId> {
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if node.inner().as_any().downcast_ref::<T>().is_some() { return Some(id); }
            }
        }
        None
    }

    pub fn find_all_by_type<T: Widget + 'static>(&self) -> Vec<(WidgetId, &T)> {
        let mut results = Vec::new();
        for id in self.traverse() {
            if let Some(node) = self.get(id) {
                if let Some(w) = node.inner().as_any().downcast_ref::<T>() {
                    results.push((id, w));
                }
            }
        }
        results
    }

    pub fn find_by_type_and_modify<T: Widget + 'static>(
        &mut self, f: impl FnOnce(&mut T),
    ) -> Option<WidgetId> {
        let id = self.find_by_type::<T>()?;
        if let Some(node) = self.get_mut(id) {
            if let Some(w) = node.inner_mut().as_any_mut().downcast_mut::<T>() { f(w); }
        }
        Some(id)
    }

    pub fn get(&self, id: WidgetId) -> Option<&BoxedWidget> {
        self.nodes.get(id).and_then(|n| n.as_ref())
    }
    pub fn get_mut(&mut self, id: WidgetId) -> Option<&mut BoxedWidget> {
        self.nodes.get_mut(id).and_then(|n| n.as_mut())
    }

    pub fn set_z_index(&mut self, id: WidgetId, z: i32) -> &mut Self {
        if let Some(n) = self.get_mut(id) { n.set_z_index(z); }
        self
    }

    pub fn add_child(&mut self, parent_id: WidgetId, child: Box<dyn Widget>) -> WidgetId {
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
        for child in children { self.add_child(child_id, child); }
        child_id
    }

    pub fn remove(&mut self, id: WidgetId) {
        let parent_id = self.nodes.get(id)
            .and_then(|n| n.as_ref()).and_then(|n| n.parent());
        if let Some(node) = self.nodes.get_mut(id) {
            if let Some(node) = node.take() {
                for child_id in node.children().to_vec() { self.remove(child_id); }
                self.free_ids.push(id);
            }
        }
        if let Some(pid) = parent_id {
            if let Some(parent) = self.get_mut(pid) {
                parent.children_mut().retain(|&c| c != id);
            }
        }
    }

    pub fn traverse(&self) -> Vec<WidgetId> {
        let mut result = Vec::new();
        if let Some(root_id) = self.root_id { self.traverse_internal(root_id, &mut result); }
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

    pub fn layout(&mut self) {
        let has_valid_root = self.root_id.and_then(|id| self.get(id))
            .map(|r| r.frame().w > 0.0 && r.frame().h > 0.0)
            .unwrap_or(false);
        if !has_valid_root {
            if let Some(root_id) = self.root_id {
                let ps = self.get(root_id).map(|r| r.preferred_size(None));
                if let Some(ps) = ps {
                    if let Some(root_mut) = self.get_mut(root_id) {
                        root_mut.set_frame(Rect::new(0.0, 0.0, ps.w.max(1.0), ps.h.max(1.0)));
                    }
                }
            }
        }

        // Phase 1: Top-down — 父容器根据 preferred_size 为子节点分配位置
        let order = self.traverse();
        for &id in &order {
            let positions: Vec<(WidgetId, Rect)> = {
                let node = match self.get(id) { Some(n) => n, None => continue, };
                let frame = node.frame();
                let children: Vec<WidgetId> = node.children().to_vec();
                if children.is_empty() { continue; }
                node.inner().layout_children(frame, &children, self)
            };
            for (child_id, rect) in positions {
                if let Some(child) = self.get_mut(child_id) {
                    let old = child.frame();
                    if old != rect {
                        child.set_frame(rect);
                        self.mark_dirty_rect(child_id, old);
                        self.mark_dirty(child_id);
                    }
                }
            }
        }

        // Phase 2: Bottom-up — 容器根据内容自动扩展高度，逐层向上传播
        for _pass in 0..3 {
            let mut any_resized = false;
            let rev_order: Vec<WidgetId> = self.traverse().into_iter().rev().collect();
            for &id in &rev_order {
                let node = match self.get(id) { Some(n) => n, None => continue };
                let children: Vec<WidgetId> = node.children().to_vec();
                if children.is_empty() { continue; }
                // 跳过 Viewport 类容器（如 ScrollView），frame 由父布局决定
                if node.inner().children_clip(node.frame()).is_some() { continue; }

                let node_frame = node.frame();
                // 取所有可见子节点的最大下边界
                let mut max_bottom = node_frame.y + node_frame.h;
                for &cid in &children {
                    if let Some(child) = self.get(cid) {
                        if child.visible() || self.get(cid).map(|c| c.children().is_empty()).unwrap_or(true) {
                            let cf = child.frame();
                            let child_bottom = cf.y + cf.h;
                            max_bottom = max_bottom.max(child_bottom);
                        }
                    }
                }

                let new_h = max_bottom - node_frame.y;
                if new_h > node_frame.h + 0.5 {
                    let old_frame = node.frame();
                    if let Some(node_mut) = self.get_mut(id) {
                        node_mut.set_frame(Rect::new(old_frame.x, old_frame.y, old_frame.w, new_h));
                        self.mark_dirty_rect(id, old_frame);
                        self.mark_dirty(id);
                    }
                    // 容器扩展后，重新布局子节点
                    let new_frame = Rect::new(old_frame.x, old_frame.y, old_frame.w, new_h);
                    let new_positions = self.get(id)
                        .map(|n| n.inner().layout_children(new_frame, &children, self))
                        .unwrap_or_default();
                    for (child_id, rect) in new_positions {
                        if let Some(child) = self.get_mut(child_id) {
                            let old = child.frame();
                            if old != rect {
                                child.set_frame(rect);
                                self.mark_dirty_rect(child_id, old);
                                self.mark_dirty(child_id);
                            }
                        }
                    }
                    any_resized = true;
                }
            }
            if !any_resized { break; }
        }
    }

    pub fn update(&mut self, dt: f32) -> bool {
        let order = self.traverse();
        let mut any_animating = false;
        for &id in &order {
            let was_animating = self.get(id)
                .map(|n| n.inner().needs_continuous_update()).unwrap_or(false);
            if let Some(node) = self.get_mut(id) { node.inner_mut().on_update(dt); }
            let (rect, scroll) = self.get(id).map(|node| {
                let is_still = node.inner().needs_continuous_update();
                let dirty = if was_animating || is_still {
                    any_animating = any_animating || is_still;
                    node.inner().dirty_rect(node.frame())
                } else { Rect::zero() };
                (dirty, node.inner().scroll_delta(node.frame()))
            }).unwrap_or_default();
            if rect.w > 0.0 || rect.h > 0.0 { self.mark_dirty_rect(id, rect); }
            if let Some((dx, dy)) = scroll {
                if dx != 0.0 || dy != 0.0 {
                    let frame = self.get(id).map(|n| n.frame()).unwrap_or_default();
                    self.scroll_deltas.push((frame, dx, dy));
                }
            }
        }
        any_animating
    }

    // ── WidgetNode tree building ──

    pub fn build(&mut self, node: WidgetNode) -> WidgetId {
        self.build_node(node, None)
    }

    fn build_node(&mut self, node: WidgetNode, parent: Option<WidgetId>) -> WidgetId {
        let id = match parent {
            Some(p) => self.add_child(p, node.widget),
            None => self.set_root(node.widget),
        };
        if let Some(n) = self.get_mut(id) { n.set_z_index(node.z_index); }
        for child in node.children { self.build_node(child, Some(id)); }
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::Point;
    use std::cell::RefCell;

    struct SpyWidget { size: crate::base::Size, last_event: RefCell<Option<WidgetEvent>> }
    impl SpyWidget {
        fn new(w: f32, h: f32) -> Self {
            Self { size: crate::base::Size::new(w, h), last_event: RefCell::new(None) }
        }
    }
    impl Widget for SpyWidget {
        fn preferred_size(&self, _: Option<&dyn crate::graphics::GraphicsEngine>) -> crate::base::Size { self.size }
        fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
            *self.last_event.borrow_mut() = Some(event.clone()); EventResult::Handled
        }
        fn render(&self, _: Rect, _: &mut crate::ui::render_context::RenderContext, _: &WidgetTree) {}
    }

    struct PassThroughContainer { size: crate::base::Size, children: RefCell<Vec<Box<dyn Widget>>> }
    impl PassThroughContainer {
        fn new(w: f32, h: f32, children: Vec<Box<dyn Widget>>) -> Self {
            Self { size: crate::base::Size::new(w, h), children: RefCell::new(children) }
        }
    }
    impl Widget for PassThroughContainer {
        fn build(&self) -> Vec<Box<dyn Widget>> { std::mem::take(&mut *self.children.borrow_mut()) }
        fn preferred_size(&self, _: Option<&dyn crate::graphics::GraphicsEngine>) -> crate::base::Size { self.size }
        fn on_event(&mut self, _: &WidgetEvent) -> EventResult { EventResult::NotHandled }
        fn render(&self, _: Rect, _: &mut crate::ui::render_context::RenderContext, _: &WidgetTree) {}
    }

    #[test] fn tree_set_root_returns_valid_id() {
        let mut tree = WidgetTree::new();
        let id = tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        assert!(tree.get(id).is_some());
        assert_eq!(tree.root().unwrap().id(), id);
    }

    #[test] fn tree_add_child_links_parent() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let cid = tree.add_child(root, Box::new(SpyWidget::new(80.0, 40.0)));
        assert!(tree.get(cid).is_some());
        assert_eq!(tree.get(root).unwrap().children(), &[cid]);
        assert_eq!(tree.get(cid).unwrap().parent(), Some(root));
    }

    #[test] fn tree_traverse_preorder() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let b = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let c = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
        assert_eq!(tree.traverse(), vec![root, a, c, b]);
    }

    #[test] fn tree_remove_cascades_to_children() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 100.0, vec![])));
        let a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 30.0)));
        let b = tree.add_child(a, Box::new(SpyWidget::new(25.0, 15.0)));
        tree.remove(a);
        assert!(tree.get(a).is_none()); assert!(tree.get(b).is_none());
        assert_eq!(tree.get(root).unwrap().children().len(), 0);
    }

    #[test] fn hit_test_root_contains() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        tree.layout();
        assert!(tree.hit_test(Point::new(50.0, 25.0)).is_some());
    }

    #[test] fn hit_test_outside_returns_none() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(100.0, 50.0)));
        tree.layout();
        assert!(tree.hit_test(Point::new(200.0, 200.0)).is_none());
        assert!(tree.hit_test(Point::new(-1.0, 25.0)).is_none());
    }

    #[test] fn hit_test_returns_deepest_child() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(child));
    }

    #[test] fn hit_test_skips_invisible() {
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        tree.get_mut(child).unwrap().set_visible(false);
        assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
    }

    #[test] fn dispatch_mouse_down_focuses_target() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let btn = tree.add_child(root_id, Box::new(SpyWidget::new(80.0, 40.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(btn).unwrap().set_frame(Rect::new(0.0, 0.0, 80.0, 40.0));
        assert_eq!(tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(40.0, 20.0), button: MouseButton::Left,
        }), EventResult::Handled);
    }

    #[test] fn dispatch_mouse_down_empty_space_clears_focus() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0))); tree.layout();
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(300.0, 300.0), button: MouseButton::Left,
        });
    }

    #[test] fn dispatch_key_to_focused_widget() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0))); tree.layout();
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 50.0), button: MouseButton::Left,
        });
        assert_eq!(tree.dispatch_event(&WidgetEvent::KeyDown { key: KeyCode::Enter }), EventResult::Handled);
    }

    #[test] fn dispatch_mouse_move_triggers_hover_enter_leave() {
        let mut tree = WidgetTree::new();
        let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
        let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
        tree.get_mut(root_id).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(child).unwrap().set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
        tree.dispatch_event(&WidgetEvent::MouseMove { pos: Point::new(50.0, 50.0) });
    }

    #[test] fn dispatch_resize_goes_to_root() {
        let mut tree = WidgetTree::new();
        tree.set_root(Box::new(SpyWidget::new(200.0, 200.0))); tree.layout();
        assert_eq!(tree.dispatch_event(&WidgetEvent::Resize { width: 400.0, height: 300.0 }), EventResult::Handled);
    }

    #[test]
    fn nav_item_click_updates_shared_active() {
        use std::cell::Cell; use std::rc::Rc;
        use crate::ui::widgets::nav::{NavItem, SharedActive};
        let active: SharedActive = Rc::new(Cell::new(0));
        let mut tree = WidgetTree::new();
        let root = tree.set_root(Box::new(
            crate::ui::widgets::Container::new().size(200.0, 200.0)
                .dir(crate::graphics::FlexDirection::Column)
        ));
        let n0 = tree.add_child(root, Box::new(NavItem::new("Item 0", 0, active.clone()).width(200.0).height(36.0)));
        let n1 = tree.add_child(root, Box::new(NavItem::new("Item 1", 1, active.clone()).width(200.0).height(36.0)));
        tree.get_mut(root).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
        tree.get_mut(n0).unwrap().set_frame(Rect::new(0.0, 0.0, 200.0, 36.0));
        tree.get_mut(n1).unwrap().set_frame(Rect::new(0.0, 36.0, 200.0, 36.0));
        assert_eq!(active.get(), 0);
        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 54.0), button: crate::base::MouseButton::Left,
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(active.get(), 1);
        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 18.0), button: crate::base::MouseButton::Left,
        });
        assert_eq!(result, EventResult::Handled);
        assert_eq!(active.get(), 0);
        let result = tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(50.0, 150.0), button: crate::base::MouseButton::Left,
        });
        assert_eq!(result, EventResult::NotHandled);
        assert_eq!(active.get(), 0);
    }
}
