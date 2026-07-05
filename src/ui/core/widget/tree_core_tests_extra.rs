// WidgetTree 单元测试（续），与 `tree_core_tests.rs` 共享同一模块作用域。

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

    tree.dispatch_event(&SystemEvent::PointerDown {
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

    let result = tree.dispatch_event(&SystemEvent::PointerDown {
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

    tree.dispatch_event(&SystemEvent::PointerDown {
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

    let result = tree.dispatch_event(&SystemEvent::KeyDown {
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

    let result = tree.dispatch_event(&SystemEvent::KeyDown {
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

    let result = tree.dispatch_event(&SystemEvent::KeyDown {
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
    let result = tree.dispatch_event(&SystemEvent::KeyDown {
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

/// PointerDown + 小幅度 PointerMove 不触发拖拽（阈值 5px）。
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

    // PointerDown
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // 小幅度移动（3px < 5px 阈值）
    tree.dispatch_event(&SystemEvent::PointerMove {
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

/// PointerDown + 大幅度 PointerMove 触发 DragStart 和 DragMove。
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

    // PointerDown
    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // 大幅度移动（超出 5px 阈值）
    tree.dispatch_event(&SystemEvent::PointerMove {
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

/// PointerUp 在拖拽激活后发射 DragEnd。
#[test]
fn drag_gesture_pointer_up_emits_drag_end() {
    let mut tree = WidgetTree::new();
    let root_id = tree.set_root(Box::new(PassThroughContainer::new(200.0, 200.0, vec![])));
    let child = tree.add_child(root_id, Box::new(SpyWidget::new(100.0, 100.0)));
    tree.get_mut(root_id)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 200.0, 200.0));
    tree.get_mut(child)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 100.0, 100.0));

    tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 50.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(60.0, 60.0),
        mods: KeyMod::NONE,
    });
    assert!(tree.drag_gesture.active, "drag should be active");

    tree.dispatch_event(&SystemEvent::PointerUp {
        pos: Point::new(65.0, 65.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    // PointerUp 后拖拽应重置
    assert!(
        !tree.drag_gesture.active,
        "drag should be reset after PointerUp"
    );
    assert!(!tree.drag_gesture.potential);
}

fn nav_item_click_updates_shared_active() {
    use crate::ui::widgets::nav::{NavItem, SharedActive};
    use std::cell::Cell;
    use std::rc::Rc;
    let active: SharedActive = Rc::new(Cell::new(0));
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(
        crate::ui::widgets::Container::new()
            .size(200.0, 200.0)
            .dir(crate::ui::layout::FlexDirection::Column),
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
    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 54.0),
        button: crate::native::MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(active.get(), 1);
    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 18.0),
        button: crate::native::MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::Handled);
    assert_eq!(active.get(), 0);
    let result = tree.dispatch_event(&SystemEvent::PointerDown {
        pos: Point::new(50.0, 150.0),
        button: crate::native::MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(result, EventResult::NotHandled);
    assert_eq!(active.get(), 0);
}
