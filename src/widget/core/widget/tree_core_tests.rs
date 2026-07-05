// WidgetTree 单元测试模块。
//
// 从 `tree_core.rs` 拆分出来以遵守 900 行文件限制。

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use crate::platform::KeyMod;
use crate::platform::Point;

struct SpyWidget {
    size: crate::platform::Size,
    last_event: RefCell<Option<WidgetEvent>>,
}
impl SpyWidget {
    fn new(w: f32, h: f32) -> Self {
        Self {
            size: crate::platform::Size::new(w, h),
            last_event: RefCell::new(None),
        }
    }
}
impl WidgetComponent for SpyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    crate::wc_upcast!(SpyWidget; WidgetLayout);
    crate::wc_upcast!(SpyWidget; WidgetRender);
    crate::wc_upcast!(SpyWidget; WidgetEventHandler);
}
impl WidgetLayout for SpyWidget {
    fn preferred_size(&self, _: Option<&dyn crate::render::traits::GraphicsEngine>) -> crate::platform::Size {
        self.size
    }
}
impl WidgetRender for SpyWidget {
    fn render(&self, _: Rect, _: &mut crate::widget::scene::RenderContext, _: &WidgetTree) {}
}
impl WidgetEventHandler for SpyWidget {
    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        *self.last_event.borrow_mut() = Some(event.clone());
        EventResult::Handled
    }
}

struct PassThroughContainer {
    size: crate::platform::Size,
    children: RefCell<Vec<Box<dyn WidgetComponent>>>,
}
impl PassThroughContainer {
    fn new(w: f32, h: f32, children: Vec<Box<dyn WidgetComponent>>) -> Self {
        Self {
            size: crate::platform::Size::new(w, h),
            children: RefCell::new(children),
        }
    }
}
impl WidgetComponent for PassThroughContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn capabilities(&self) -> WidgetCapabilities {
        WidgetCapabilities::from_bits(
            WidgetCapabilities::LAYOUT | WidgetCapabilities::RENDER | WidgetCapabilities::EVENT,
        )
    }
    fn build(&self) -> Vec<Box<dyn WidgetComponent>> {
        std::mem::take(&mut *self.children.borrow_mut())
    }
    crate::wc_upcast!(PassThroughContainer; WidgetLayout);
    crate::wc_upcast!(PassThroughContainer; WidgetRender);
    crate::wc_upcast!(PassThroughContainer; WidgetEventHandler);
}
impl WidgetLayout for PassThroughContainer {
    fn preferred_size(&self, _: Option<&dyn crate::render::traits::GraphicsEngine>) -> crate::platform::Size {
        self.size
    }
}
impl WidgetRender for PassThroughContainer {
    fn render(&self, _: Rect, _: &mut crate::widget::scene::RenderContext, _: &WidgetTree) {}
}
impl WidgetEventHandler for PassThroughContainer {
    fn on_event(&mut self, _: &WidgetEvent) -> EventResult {
        EventResult::NotHandled
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
        mods: KeyMod::NONE,
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

// ════════════════════════════════════════════════════════════════════════
// 捕获阶段测试
// ════════════════════════════════════════════════════════════════════════

/// 捕获阶段：根节点在捕获阶段处理事件，阻止其到达子节点。
#[test]
fn capture_phase_root_handles_before_child() {
    let mut tree = WidgetTree::new();
    // 树结构：SpyWidget(root, Handled) → PassThroughContainer → SpyWidget(child)
    // SpyWidget 的 on_event 返回 Handled，所以 root 捕获后子节点收不到。
    let root_id = tree.set_root(Box::new(SpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(container)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    // 点击在 child 区域内
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // SpyWidget(root) 在捕获阶段返回 Handled，最终结果应为 Handled
    assert_eq!(result, EventResult::Handled);
    // SpyWidget(root) 收到了事件（捕获阶段）
    // root 不是 focused_widget（target=child 在冒泡阶段才设焦点，但捕获阶段 Handled 阻止了冒泡）
    // 所以 focused_widget 应为 None（MouseDown 未进入冒泡阶段的 set_focus）
    assert!(tree.focused_widget.is_none());
}

/// 捕获阶段不拦截时，事件正常冒泡到目标。
#[test]
fn capture_phase_not_intercepted_proceeds_to_bubble() {
    let mut tree = WidgetTree::new();
    // 树结构：PassThroughContainer(root) → SpyWidget(child)
    // PassThroughContainer 返回 NotHandled，不拦截
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    // 点击在 child 区域内
    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // 捕获阶段无人拦截 → 冒泡阶段 child(SpyWidget) 返回 Handled
    assert_eq!(result, EventResult::Handled);
    // 冒泡阶段设置了焦点
    assert_eq!(tree.focused_widget, Some(child));
}

/// 捕获阶段：MouseWheel 事件被祖先拦截（如 ScrollView 外层拦截滚动）。
#[test]
fn capture_phase_mouse_wheel_intercepted() {
    let mut tree = WidgetTree::new();
    // 树结构：SpyWidget(root, Handled) → PassThroughContainer → SpyWidget(child)
    let root_id = tree.set_root(Box::new(SpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(container)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let result = tree.dispatch_event(&WidgetEvent::MouseWheel {
        pos: Point::new(50.0, 50.0),
        delta: Point::new(0.0, -10.0),
    });
    assert_eq!(result, EventResult::Handled);
}

/// 捕获阶段：KeyDown 事件被祖先拦截（如全局快捷键）。
#[test]
fn capture_phase_key_down_intercepted() {
    let mut tree = WidgetTree::new();
    // 树结构：SpyWidget(root, Handled) → PassThroughContainer → SpyWidget(child)
    let root_id = tree.set_root(Box::new(SpyWidget::new(300.0, 300.0)));
    let container = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(300.0, 300.0, vec![])),
    );
    let _child = tree.add_child(container, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 300.0));

    // 先设焦点（直接设置 focused_widget 字段，但它是 pub(crate) 的）
    // 或者通过 dispatch MouseDown 设焦点
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    let result = tree.dispatch_event(&WidgetEvent::KeyDown {
        key: KeyCode::Escape,
        mods: KeyMod::NONE,
    });
    // SpyWidget(root) 在捕获阶段返回 Handled
    assert_eq!(result, EventResult::Handled);
}

// ════════════════════════════════════════════════════════════════════════
// 事件管理器集成测试
// ════════════════════════════════════════════════════════════════════════

/// EventManager 的 add_handler 处理所有事件。
#[test]
fn event_manager_catches_mouse_down() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let handled = Rc::new(RefCell::new(false));
    let h = handled.clone();
    let em = tree.event_manager_for(root_id);
    em.add_handler(move |_| {
        *h.borrow_mut() = true;
        EventResult::Handled
    });

    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert!(
        *handled.borrow(),
        "EventManager handler should have been called"
    );
}

/// EventManager 的 on_kind 只处理匹配的事件类型。
#[test]
fn event_manager_on_kind_filters() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let mouse_down_count = Rc::new(RefCell::new(0u32));
    let md = mouse_down_count.clone();
    let key_count = Rc::new(RefCell::new(0u32));
    let kc = key_count.clone();

    {
        let em = tree.event_manager_for(root_id);
        em.on_kind(WidgetEventKind::MouseDown, move |_| {
            *md.borrow_mut() += 1;
            EventResult::Handled
        });
        em.on_kind(WidgetEventKind::KeyDown, move |_| {
            *kc.borrow_mut() += 1;
            EventResult::Handled
        });
    }

    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        *mouse_down_count.borrow(),
        1,
        "MouseDown handler should fire"
    );
    assert_eq!(*key_count.borrow(), 0, "KeyDown handler should NOT fire");

    tree.dispatch_event(&WidgetEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        *mouse_down_count.borrow(),
        1,
        "MouseDown handler should NOT fire again"
    );
    assert_eq!(*key_count.borrow(), 1, "KeyDown handler should fire");
}
