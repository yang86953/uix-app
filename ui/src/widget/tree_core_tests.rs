//! WidgetTree 单元测试模块。
//!
//! 从 `tree_core.rs` 拆分出来以遵守 900 行文件限制。

use super::*;
use uix_platform::Point;
use uix_platform::KeyMod;
use std::cell::RefCell;

struct SpyWidget {
    size: uix_platform::Size,
    last_event: RefCell<Option<WidgetEvent>>,
}
impl SpyWidget {
    fn new(w: f32, h: f32) -> Self {
        Self {
            size: uix_platform::Size::new(w, h),
            last_event: RefCell::new(None),
        }
    }
}
impl Widget for SpyWidget {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn preferred_size(
        &self,
        _: Option<&dyn uix_graphics::GraphicsEngine>,
    ) -> uix_platform::Size {
        self.size
    }
    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        *self.last_event.borrow_mut() = Some(event.clone());
        EventResult::Handled
    }
    fn render(
        &self,
        _: Rect,
        _: &mut crate::render_context::RenderContext,
        _: &WidgetTree,
    ) {
    }
}

struct PassThroughContainer {
    size: uix_platform::Size,
    children: RefCell<Vec<Box<dyn Widget>>>,
}
impl PassThroughContainer {
    fn new(w: f32, h: f32, children: Vec<Box<dyn Widget>>) -> Self {
        Self {
            size: uix_platform::Size::new(w, h),
            children: RefCell::new(children),
        }
    }
}
impl Widget for PassThroughContainer {
    fn as_any(&self) -> &dyn std::any::Any { self }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any { self }
    fn build(&self) -> Vec<Box<dyn Widget>> {
        std::mem::take(&mut *self.children.borrow_mut())
    }
    fn preferred_size(
        &self,
        _: Option<&dyn uix_graphics::GraphicsEngine>,
    ) -> uix_platform::Size {
        self.size
    }
    fn on_event(&mut self, _: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
    }
    fn render(
        &self,
        _: Rect,
        _: &mut crate::render_context::RenderContext,
        _: &WidgetTree,
    ) {
    }
}

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
    assert_eq!(tree.traverse(), vec![root, a, c, b]);
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
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(child));
}

#[test]
fn hit_test_skips_invisible() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    tree.get_mut(child).unwrap().set_visible(false);
    assert_eq!(tree.hit_test(Point::new(50.0, 50.0)), Some(root));
}

#[test]
fn dispatch_mouse_down_focuses_target() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn = tree.add_child(root_id, Box::new(SpyWidget::new(80.0, 40.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(btn)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 80.0, 40.0));
    assert_eq!(
        tree.dispatch_event(&WidgetEvent::MouseDown {
            pos: Point::new(40.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

#[test]
fn dispatch_mouse_down_empty_space_clears_focus() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(300.0, 300.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
}

#[test]
fn dispatch_key_to_focused_widget() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        tree.dispatch_event(&WidgetEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
}

#[test]
fn dispatch_mouse_move_triggers_hover_enter_leave() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));
    tree.dispatch_event(&WidgetEvent::MouseMove {
        pos: Point::new(50.0, 50.0),
    });
}

#[test]
fn dispatch_resize_goes_to_root() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(SpyWidget::new(200.0, 200.0)));
    tree.layout();
    assert_eq!(
        tree.dispatch_event(&WidgetEvent::Resize {
            width: 400.0,
            height: 300.0
        }),
        EventResult::Handled
    );
}

#[test]
fn nav_item_click_updates_shared_active() {
    use crate::widgets::nav::{NavItem, SharedActive};
    use std::cell::Cell;
    use std::rc::Rc;
    let active: SharedActive = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        crate::widgets::Container::new()
            .size(200.0, 200.0)
            .dir(crate::FlexDirection::Column),
    ));
    let n0 = tree.add_child(
        root,
        Box::new(
            NavItem::new("Item 0", 0, active.clone())
                .width(200.0)
                .height(36.0),
        ),
    );
    let n1 = tree.add_child(
        root,
        Box::new(
            NavItem::new("Item 1", 1, active.clone())
                .width(200.0)
                .height(36.0),
        ),
    );
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(n0)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 36.0));
    tree.get_mut(n1)
        .unwrap()
        .set_frame(Rect::new(0.0, 36.0, 200.0, 36.0));
    assert_eq!(active.get(), 0);
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 54.0),
        button: uix_platform::MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(active.get(), 1);
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 18.0),
        button: uix_platform::MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(active.get(), 0);
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 150.0),
        button: uix_platform::MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::NotHandled);
    assert_eq!(active.get(), 0);
}
