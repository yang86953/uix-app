//! WidgetTree 单元测试模块。
//!
//! 从 `tree_core.rs` 拆分出来以遵守 900 行文件限制。

use super::*;
use std::cell::RefCell;
use std::rc::Rc;
use uix_platform::KeyMod;
use uix_platform::Point;

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
    fn preferred_size(&self, _: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> uix_platform::Size {
        self.size
    }
}
impl WidgetRender for SpyWidget {
    fn render(&self, _: Rect, _: &mut crate::render_context::RenderContext, _: &WidgetTree) {}
}
impl WidgetEventHandler for SpyWidget {
    fn on_event(&mut self, event: &WidgetEvent) -> EventResult {
        *self.last_event.borrow_mut() = Some(event.clone());
        EventResult::Handled
    }
}

struct PassThroughContainer {
    size: uix_platform::Size,
    children: RefCell<Vec<Box<dyn WidgetComponent>>>,
}
impl PassThroughContainer {
    fn new(w: f32, h: f32, children: Vec<Box<dyn WidgetComponent>>) -> Self {
        Self {
            size: uix_platform::Size::new(w, h),
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
    fn preferred_size(&self, _: Option<&dyn uix_graphics::traits::GraphicsEngine>) -> uix_platform::Size {
        self.size
    }
}
impl WidgetRender for PassThroughContainer {
    fn render(&self, _: Rect, _: &mut crate::render_context::RenderContext, _: &WidgetTree) {}
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

/// EventManager 优先级：高优先级 handler 先执行。
#[test]
fn event_manager_priority_order() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let order = Rc::new(RefCell::new(Vec::<u32>::new()));
    let o1 = order.clone();
    let o2 = order.clone();
    let o3 = order.clone();

    {
        let em = tree.event_manager_for(root_id);
        em.add_handler_with_priority(0, move |_| {
            o1.borrow_mut().push(1);
            EventResult::NotHandled
        });
        em.add_handler_with_priority(10, move |_| {
            o2.borrow_mut().push(2);
            EventResult::NotHandled
        });
        em.add_handler_with_priority(-5, move |_| {
            o3.borrow_mut().push(3);
            EventResult::NotHandled
        });
    }

    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    let v = order.borrow().clone();
    assert_eq!(v, vec![2, 1, 3], "priority order: 10, 0, -5");
}

/// EventManager 在 widget on_event(NotHandled) 之后执行，返回 Handled 阻止冒泡到父节点。
#[test]
fn event_manager_handled_stops_bubble_to_parent() {
    let mut tree = WidgetTree::new();
    // 树：PassThroughContainer(root) → PassThroughContainer(child)
    // 两者 on_event 都返回 NotHandled
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(100.0, 100.0, vec![])),
    );
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    let parent_em_called = Rc::new(RefCell::new(false));
    let pc = parent_em_called.clone();

    // child 的 EventManager 返回 Handled，阻止冒泡到 parent
    {
        let em = tree.event_manager_for(child);
        em.add_handler(move |_| EventResult::Handled);
    }
    // parent 的 EventManager：不应被执行（因为 child 的 EventManager 已 Handled）
    {
        let em = tree.event_manager_for(root_id);
        em.add_handler(move |_| {
            *pc.borrow_mut() = true;
            EventResult::Handled
        });
    }

    let result = tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    // child 的 on_event 返回 NotHandled，EventManager 返回 Handled
    // 所以结果是 Handled
    assert_eq!(result, EventResult::Handled);
    // parent 的 EventManager 不应被调用
    assert!(
        !*parent_em_called.borrow(),
        "parent EventManager should NOT be called when child EventManager handled"
    );
}

/// EventManager 在 widget on_event 之后、冒泡之前执行。
#[test]
fn event_manager_runs_after_on_event() {
    let mut tree = WidgetTree::new();
    // 使用 PassThroughContainer（on_event 返回 NotHandled）
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));

    let order = Rc::new(RefCell::new(Vec::<String>::new()));
    let o1 = order.clone();
    let o2 = order.clone();

    {
        let em = tree.event_manager_for(root_id);
        em.add_handler(move |_| {
            o1.borrow_mut().push("manager".into());
            EventResult::Handled
        });
    }
    // 在根节点再挂一个子 widget，确保事件经过它
    let child = tree.add_child(
        root_id,
        Box::new(PassThroughContainer::new(50.0, 50.0, vec![])),
    );
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 50.0, 50.0));

    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(25.0, 25.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    // 事件管理器确实被调用了
    assert!(!order.borrow().is_empty());
}

// ════════════════════════════════════════════════════════════════════════
// 焦点导航测试
// ════════════════════════════════════════════════════════════════════════

/// 没有 widget 设置 tab_index 时，collect_focusable 返回空。
#[test]
fn collect_focusable_empty_by_default() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    tree.layout();
    let focusable = tree.collect_focusable();
    assert!(
        focusable.is_empty(),
        "no tab_index set → no focusable widgets"
    );
}

/// 设置 tab_index > 0 的 widget 可被收集。
#[test]
fn collect_focusable_returns_widgets_with_tab_index() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn1 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    let btn2 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    tree.get_mut(btn1).unwrap().set_tab_index(1);
    tree.get_mut(btn2).unwrap().set_tab_index(2);
    tree.layout();
    let focusable = tree.collect_focusable();
    assert_eq!(focusable.len(), 2);
    assert_eq!(focusable[0], btn1, "tab_index=1 first");
    assert_eq!(focusable[1], btn2, "tab_index=2 second");
}

/// collect_focusable 按 tab_index 升序排序。
#[test]
fn collect_focusable_sorted_by_tab_index() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn_a = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0))); // tab_index=3
    let btn_b = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0))); // tab_index=1
    let btn_c = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0))); // tab_index=2
    tree.get_mut(btn_a).unwrap().set_tab_index(3);
    tree.get_mut(btn_b).unwrap().set_tab_index(1);
    tree.get_mut(btn_c).unwrap().set_tab_index(2);
    tree.layout();
    let focusable = tree.collect_focusable();
    assert_eq!(
        focusable,
        vec![btn_b, btn_c, btn_a],
        "sorted by tab_index ascending"
    );
}

/// Tab 键聚焦到下一个可聚焦 widget。
#[test]
fn tab_key_focuses_next_widget() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn1 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    let btn2 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    let btn3 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    tree.get_mut(btn1).unwrap().set_tab_index(1);
    tree.get_mut(btn2).unwrap().set_tab_index(2);
    tree.get_mut(btn3).unwrap().set_tab_index(3);
    // 设焦点在 btn1
    tree.focused_widget = Some(btn1);

    let result = tree.dispatch_event(&WidgetEvent::KeyDown {
        key: KeyCode::Tab,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled, "Tab should be handled");
    assert_eq!(tree.focused_widget, Some(btn2), "focus should move to btn2");
}

/// Shift+Tab 聚焦到上一个可聚焦 widget。
#[test]
fn shift_tab_focuses_prev_widget() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn1 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    let btn2 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    let btn3 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    tree.get_mut(btn1).unwrap().set_tab_index(1);
    tree.get_mut(btn2).unwrap().set_tab_index(2);
    tree.get_mut(btn3).unwrap().set_tab_index(3);
    // 设焦点在 btn2
    tree.focused_widget = Some(btn2);

    let result = tree.dispatch_event(&WidgetEvent::KeyDown {
        key: KeyCode::Tab,
        mods: KeyMod::SHIFT,
    });
    assert_eq!(result, EventResult::Handled, "Shift+Tab should be handled");
    assert_eq!(tree.focused_widget, Some(btn1), "focus should move to btn1");
}

/// Tab 在最后一个 widget 时循环到第一个。
#[test]
fn tab_wraps_around_to_first() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let btn1 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    let btn2 = tree.add_child(root, Box::new(SpyWidget::new(50.0, 50.0)));
    tree.get_mut(btn1).unwrap().set_tab_index(1);
    tree.get_mut(btn2).unwrap().set_tab_index(2);
    // 设焦点在 btn2（最后一个）
    tree.focused_widget = Some(btn2);

    let result = tree.dispatch_event(&WidgetEvent::KeyDown {
        key: KeyCode::Tab,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(
        tree.focused_widget,
        Some(btn1),
        "Tab at last should wrap to first"
    );
}

/// 无可聚焦 widget 时，Tab 不产生焦点变化。
#[test]
fn tab_no_focusable_does_nothing() {
    let mut tree = WidgetTree::new();
    tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    tree.layout();
    let result = tree.dispatch_event(&WidgetEvent::KeyDown {
        key: KeyCode::Tab,
        mods: KeyMod::NONE,
    });
    assert_eq!(
        result,
        EventResult::NotHandled,
        "Tab with no focusable should be NotHandled"
    );
}

// ════════════════════════════════════════════════════════════════════════
// 拖拽手势测试
// ════════════════════════════════════════════════════════════════════════

/// MouseDown + 小幅度 MouseMove 不触发拖拽（阈值 5px）。
#[test]
fn drag_gesture_threshold_not_exceeded() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    // MouseDown
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // 小幅度移动（3px < 5px 阈值）
    tree.dispatch_event(&WidgetEvent::MouseMove {
        pos: Point::new(53.0, 50.0),
        mods: KeyMod::NONE,
    });
    // 拖拽未激活
    assert!(
        !tree.drag_gesture.active,
        "drag should not start below threshold"
    );
    assert!(
        tree.drag_gesture.potential,
        "still potential after small move"
    );
}

/// MouseDown + 大幅度 MouseMove 触发 DragStart 和 DragMove。
#[test]
fn drag_gesture_triggers_drag_start() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    // MouseDown
    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // 大幅度移动（超出 5px 阈值）
    tree.dispatch_event(&WidgetEvent::MouseMove {
        pos: Point::new(60.0, 60.0),
        mods: KeyMod::NONE,
    });
    // 拖拽应激活
    assert!(
        tree.drag_gesture.active,
        "drag should be active after threshold exceeded"
    );
    assert!(
        !tree.drag_gesture.potential,
        "no longer potential after drag start"
    );
}

/// MouseUp 在拖拽激活后发射 DragEnd。
#[test]
fn drag_gesture_mouse_up_emits_drag_end() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    tree.dispatch_event(&WidgetEvent::MouseDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&WidgetEvent::MouseMove {
        pos: Point::new(60.0, 60.0),
        mods: KeyMod::NONE,
    });
    assert!(tree.drag_gesture.active, "drag should be active");

    tree.dispatch_event(&WidgetEvent::MouseUp {
        pos: Point::new(65.0, 65.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // MouseUp 后拖拽应重置
    assert!(
        !tree.drag_gesture.active,
        "drag should be reset after MouseUp"
    );
    assert!(!tree.drag_gesture.potential);
}

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
