use crate::tests::common::*;
use crate::ui::view::{embed, ViewAdapter};
use crate::ui::widgets::feedback::TriggerMode;
use crate::ui::widgets::general::float_button::*;
use crate::ui::{
    AccessibilityRole, EventHandler, LayoutChild, WidgetAnimation, WidgetComponent, WidgetLayout,
    WidgetRender,
};
use std::cell::Cell;
use std::rc::Rc;

fn group_pointer_event(pos: Point, button: MouseButton, down: bool) -> SystemEvent {
    if down {
        SystemEvent::PointerDown {
            pos,
            button,
            mods: KeyMod::NONE,
        }
    } else {
        SystemEvent::PointerUp {
            pos,
            button,
            mods: KeyMod::NONE,
        }
    }
}

fn group_key_event(key: KeyCode, down: bool) -> SystemEvent {
    if down {
        SystemEvent::KeyDown {
            key,
            mods: KeyMod::NONE,
        }
    } else {
        SystemEvent::KeyUp {
            key,
            mods: KeyMod::NONE,
        }
    }
}

fn assert_group_state(group: &FloatButtonGroup, expected: bool, expected_count: usize) {
    let fields = WidgetComponent::snapshot_fields(group);
    assert!(matches!(
        fields,
        SnapshotFields::FloatButtonGroup {
            button_count,
            expanded,
            ..
        } if button_count == expected_count && expanded == expected
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.state.expanded, Some(expected));
}

#[test]
fn float_button_icon_is_rendered_only_through_the_icon_component_pipeline() {
    let source = include_str!("../../../../ui/widgets/general/float_button.rs");

    assert!(source.contains("Icon::paint_in_frame"));
    assert!(!source.contains("draw_text(\n                &self.icon"));
    assert!(!source.contains("Self::new(\"+\")"));
}

#[test]
fn measure_preserves_float_button_zero_layout_footprint() {
    let measured = FloatButton::new("plus").measure(Constraints::loose(Size::new(40.0, 40.0)));

    assert_eq!(measured, Size::zero());
}

#[test]
fn reserved_layout_space_matches_the_button_diameter() {
    let button = FloatButton::new("plus")
        .size(48.0)
        .reserve_layout_space(true);
    let measured = button.measure(Constraints::loose(Size::new(80.0, 80.0)));

    assert_eq!(measured, Size::new(48.0, 48.0));
    assert!(WidgetRender::overlay_entry(&button, ComponentId::new(5), Rect::zero()).is_none());
}

#[test]
fn float_button_exposes_real_hit_damage_overlay_and_accessibility_bounds() {
    let button = FloatButton::new("plus")
        .tooltip("新建")
        .size(48.0)
        .position(12.0, 8.0);
    let frame = Rect::new(100.0, 50.0, 0.0, 0.0);
    let hit = EventHandler::hit_test_frame(&button, frame);
    assert_eq!(hit, Rect::new(112.0, 58.0, 48.0, 48.0));
    assert!(WidgetRender::dirty_rect(&button, frame).contains(Point::new(115.0, 60.0)));
    let overlay =
        WidgetRender::overlay_entry(&button, ComponentId::new(4), frame).expect("float overlay");
    assert_eq!(overlay.kind(), crate::ui::OverlayKind::Custom);
    assert_eq!(overlay.bounds_rect(), Some(hit));
    assert_eq!(
        button.snapshot_fields().accessibility().role,
        AccessibilityRole::Button
    );

    let mut tree = WidgetTree::new();
    let id = tree.set_root(Box::new(button));
    tree.get_mut(id).expect("float root").set_frame(frame);
    assert_eq!(tree.visible_rect_for(id), Some(hit));
}

#[test]
fn pointer_and_keyboard_activation_reach_public_click_handler() {
    let calls = Rc::new(Cell::new(0));
    let observed = Rc::clone(&calls);
    let mut tree = ViewAdapter::build(embed(FloatButton::new("plus").tooltip("新建")).on_click_fn(
        move || {
            observed.set(observed.get() + 1);
        },
    ));
    let id = tree.root_id().expect("float root");
    tree.get_mut(id)
        .expect("float node")
        .set_frame(Rect::zero());
    let pos = Point::new(20.0, 20.0);

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerUp {
            pos,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);

    tree.set_focus(Some(id));
    assert_eq!(tree.get(id).expect("float node").component().tab_index(), 1);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(calls.get(), 2);
}

#[test]
fn click_group_requires_a_matching_trigger_release_and_cancels_on_leave() {
    let inside = Point::new(20.0, 20.0);
    let outside = Point::new(60.0, 20.0);
    let mut group = FloatButtonGroup::new()
        .buttons(vec![FloatButton::new("A"), FloatButton::new("B")])
        .trigger(TriggerMode::Click);

    assert_eq!(WidgetComponent::tab_index(&group), 1);
    assert_eq!(
        group.on_event(&group_pointer_event(inside, MouseButton::Left, true)),
        EventResult::Handled
    );
    assert_group_state(&group, false, 2);
    assert_eq!(
        group.on_event(&group_pointer_event(outside, MouseButton::Left, false)),
        EventResult::Handled
    );
    assert_group_state(&group, false, 2);

    group.on_event(&group_pointer_event(inside, MouseButton::Left, true));
    assert_eq!(
        group.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_eq!(
        group.on_event(&group_pointer_event(inside, MouseButton::Left, false)),
        EventResult::NotHandled
    );
    assert_group_state(&group, false, 2);

    group.on_event(&group_pointer_event(inside, MouseButton::Left, true));
    group.on_event(&group_pointer_event(inside, MouseButton::Left, false));
    assert_group_state(&group, true, 2);

    group.on_event(&group_pointer_event(inside, MouseButton::Left, true));
    group.on_event(&group_pointer_event(inside, MouseButton::Left, false));
    assert_group_state(&group, false, 2);
}

#[test]
fn group_trigger_modes_execute_distinct_hover_focus_and_context_menu_paths() {
    let inside = Point::new(12.0, 12.0);
    let mut hover = FloatButtonGroup::new().buttons(vec![FloatButton::new("A")]);
    assert_eq!(
        hover.on_event(&SystemEvent::PointerEnter),
        EventResult::Handled
    );
    assert_group_state(&hover, true, 1);
    assert_eq!(
        hover.on_event(&SystemEvent::PointerLeave),
        EventResult::Handled
    );
    assert_group_state(&hover, false, 1);

    let mut focus = FloatButtonGroup::new()
        .buttons(vec![FloatButton::new("A")])
        .trigger(TriggerMode::Focus);
    assert_eq!(
        EventHandler::on_focus_within(&mut focus, true),
        EventResult::Handled
    );
    assert_group_state(&focus, true, 1);
    assert_eq!(
        EventHandler::on_focus_within(&mut focus, false),
        EventResult::Handled
    );
    assert_group_state(&focus, false, 1);

    let mut context = FloatButtonGroup::new()
        .buttons(vec![FloatButton::new("A")])
        .trigger(TriggerMode::ContextMenu);
    assert_eq!(
        context.on_event(&group_pointer_event(inside, MouseButton::Left, true)),
        EventResult::NotHandled
    );
    assert_group_state(&context, false, 1);
    assert_eq!(
        context.on_event(&group_pointer_event(inside, MouseButton::Right, true)),
        EventResult::Handled
    );
    assert_eq!(
        context.on_event(&group_pointer_event(inside, MouseButton::Right, false)),
        EventResult::Handled
    );
    assert_group_state(&context, true, 1);
}

#[test]
fn group_keyboard_activation_matches_keys_and_focus_loss_cancels_pending_press() {
    let mut group = FloatButtonGroup::new()
        .buttons(vec![FloatButton::new("A")])
        .trigger(TriggerMode::Click);

    group.on_event(&group_key_event(KeyCode::Enter, true));
    assert_eq!(
        group.on_event(&group_key_event(KeyCode::Space, false)),
        EventResult::Handled
    );
    assert_group_state(&group, false, 1);

    group.on_event(&group_key_event(KeyCode::Enter, true));
    group.on_event(&SystemEvent::FocusOut);
    assert_eq!(
        group.on_event(&group_key_event(KeyCode::Enter, false)),
        EventResult::NotHandled
    );
    assert_group_state(&group, false, 1);

    group.on_event(&SystemEvent::FocusIn);
    group.on_event(&group_key_event(KeyCode::Space, true));
    group.on_event(&group_key_event(KeyCode::Space, false));
    assert_group_state(&group, true, 1);
    assert_eq!(
        group.on_event(&group_key_event(KeyCode::Escape, true)),
        EventResult::Handled
    );
    assert_group_state(&group, false, 1);

    let mut empty = FloatButtonGroup::new().trigger(TriggerMode::Click);
    assert_eq!(WidgetComponent::tab_index(&empty), 0);
    assert_eq!(
        empty.on_event(&group_key_event(KeyCode::Enter, true)),
        EventResult::NotHandled
    );
    assert_group_state(&empty, false, 0);
}

#[test]
fn group_animation_keeps_child_visibility_layout_hit_testing_and_damage_in_sync() {
    let mut group = FloatButtonGroup::new().buttons(vec![
        FloatButton::new("A").size(32.0),
        FloatButton::new("B").tooltip("第二项").size(48.0),
    ]);
    let frame = Rect::new(100.0, 80.0, 40.0, 40.0);
    let children = [
        LayoutChild::new(ComponentId::new(2), Size::zero()),
        LayoutChild::new(ComponentId::new(3), Size::zero()),
    ];

    assert_eq!(
        WidgetLayout::measure(&group, Constraints::loose(Size::new(300.0, 300.0))),
        Size::new(40.0, 40.0)
    );
    assert!(!WidgetLayout::child_visible(&group, 0));
    assert!(!EventHandler::hit_test_children(&group));

    group.on_event(&SystemEvent::PointerEnter);
    assert_group_state(&group, true, 2);
    assert!(!WidgetLayout::child_visible(&group, 0));
    assert!(WidgetAnimation::update_animation(&mut group, 0.05));
    let moving = WidgetLayout::layout_children(&group, frame, &children, &WidgetTree::new());
    assert!(WidgetLayout::child_visible(&group, 0));
    assert!(EventHandler::hit_test_children(&group));
    assert_eq!(moving.len(), 2);
    assert!(moving[0].1.y > frame.y);
    assert!(moving[1].1.y > moving[0].1.y);
    let moving_center = Point::new(
        moving[0].1.x + moving[0].1.w * 0.5,
        moving[0].1.y + moving[0].1.h * 0.5,
    );
    assert!(EventHandler::hit_test_frame(&group, frame).contains(moving_center));
    let dirty = WidgetAnimation::dirty_bounds(&group, frame);
    assert!(dirty.w > frame.w && dirty.h > frame.h);
    assert!(dirty.contains(moving_center));

    assert!(!WidgetAnimation::update_animation(&mut group, 1.0));
    let settled = WidgetLayout::layout_children(&group, frame, &children, &WidgetTree::new());
    assert!(settled[0].1.y > moving[0].1.y);
    assert!(settled[1].1.y >= settled[0].1.y + settled[0].1.h + 8.0);

    group.on_event(&SystemEvent::PointerLeave);
    assert_group_state(&group, false, 2);
    assert!(WidgetAnimation::update_animation(&mut group, 0.05));
    assert!(WidgetLayout::child_visible(&group, 1));
    assert!(EventHandler::hit_test_children(&group));
    assert!(!WidgetAnimation::update_animation(&mut group, 1.0));
    assert!(!WidgetLayout::child_visible(&group, 1));
    assert!(!EventHandler::hit_test_children(&group));
    assert_eq!(EventHandler::hit_test_frame(&group, frame), frame);
}

#[test]
fn group_materializes_children_and_tree_hit_testing_tracks_expansion() {
    let mut tree = ViewAdapter::build(
        FloatButtonGroup::new().buttons(vec![FloatButton::new("A"), FloatButton::new("B")]),
    );
    tree.layout();
    let root = tree.root_id().expect("group root");
    let children = tree.get(root).expect("group node").children().to_vec();
    assert_eq!(children.len(), 2);
    assert!(children
        .iter()
        .all(|child| !tree.get(*child).expect("group child").visible()));

    let _ = tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(20.0, 20.0),
        mods: KeyMod::NONE,
    });
    assert!(tree.update_animations(0.08).len() <= 1);
    tree.layout();
    assert!(children
        .iter()
        .all(|child| tree.get(*child).expect("group child").visible()));
    let first_frame = tree.get(children[0]).expect("first child").frame();
    let first_center = Point::new(
        first_frame.x + first_frame.w * 0.5,
        first_frame.y + first_frame.h * 0.5,
    );
    assert_eq!(tree.hit_test(first_center), Some(children[0]));

    let _ = tree.dispatch_event(&SystemEvent::PointerMove {
        pos: Point::new(500.0, 500.0),
        mods: KeyMod::NONE,
    });
    let group = tree
        .get(root)
        .expect("group root")
        .component()
        .as_any()
        .downcast_ref::<FloatButtonGroup>()
        .expect("group component");
    assert_group_state(group, false, 2);
    let _ = tree.update_animations(1.0);
    tree.layout();
    assert!(children
        .iter()
        .all(|child| !tree.get(*child).expect("group child").visible()));
    assert_ne!(tree.hit_test(first_center), Some(children[0]));
}

#[test]
fn reconcile_reuses_group_and_children_while_preserving_expanded_runtime_state() {
    let mut tree = ViewAdapter::build(
        FloatButtonGroup::new()
            .buttons(vec![FloatButton::new("A"), FloatButton::new("B")])
            .trigger(TriggerMode::Click),
    );
    tree.layout();
    let root = tree.root_id().expect("group root");
    let before_children = tree.get(root).expect("group node").children().to_vec();
    let before_ptr = tree
        .get(root)
        .expect("group node")
        .component()
        .as_any()
        .downcast_ref::<FloatButtonGroup>()
        .expect("group component") as *const FloatButtonGroup;

    let inside = Point::new(20.0, 20.0);
    assert_eq!(
        tree.dispatch_event(&group_pointer_event(inside, MouseButton::Left, true)),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&group_pointer_event(inside, MouseButton::Left, false)),
        EventResult::Handled
    );
    let group = tree
        .get(root)
        .expect("group node")
        .component()
        .as_any()
        .downcast_ref::<FloatButtonGroup>()
        .expect("group component");
    assert_group_state(group, true, 2);

    ViewAdapter::reconcile(
        &mut tree,
        FloatButtonGroup::new()
            .buttons(vec![
                FloatButton::new("A2").size(44.0),
                FloatButton::new("B2"),
                FloatButton::new("C"),
            ])
            .trigger(TriggerMode::Click),
    );

    let group = tree
        .get(root)
        .expect("group node")
        .component()
        .as_any()
        .downcast_ref::<FloatButtonGroup>()
        .expect("group component");
    assert_eq!(group as *const FloatButtonGroup, before_ptr);
    assert_group_state(group, true, 3);
    let after_children = tree.get(root).expect("group node").children();
    assert_eq!(after_children.len(), 3);
    assert_eq!(&after_children[..2], before_children.as_slice());
}
