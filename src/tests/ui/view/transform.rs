use std::cell::RefCell;
use std::rc::Rc;

use crate::draw::compositor::LayerTree;
use crate::tests::common::*;
use crate::ui::animation::AnimationConfig;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{canvas, column, label, ViewAdapter};
use crate::ui::{EventResult, Placement, SystemEvent};

#[test]
fn static_transform_drives_hit_testing_events_and_semantic_bounds() {
    let observed_pointer = Rc::new(RefCell::new(None));
    let pointer_capture = Rc::clone(&observed_pointer);
    let observed_wheel = Rc::new(RefCell::new(None));
    let wheel_capture = Rc::clone(&observed_wheel);
    let mut tree = ViewAdapter::build(
        label("target")
            .width(20.0)
            .height(10.0)
            .offset(Point::new(30.0, 10.0))
            .scale(2.0)
            .on_event(move |event| match event {
                SystemEvent::PointerDown { pos, .. } => {
                    *pointer_capture.borrow_mut() = Some(*pos);
                    EventResult::Handled
                }
                SystemEvent::Wheel { pos, delta } => {
                    *wheel_capture.borrow_mut() = Some((*pos, *delta));
                    EventResult::Handled
                }
                _ => EventResult::NotHandled,
            }),
    );
    let root = tree.root_id().expect("root");
    tree.get_mut(root)
        .expect("root node")
        .set_frame(Rect::new(10.0, 10.0, 20.0, 10.0));
    tree.reconcile_lifecycle_after_layout();

    let visual_center = Point::new(50.0, 25.0);
    assert_eq!(tree.hit_test(visual_center), Some(root));
    assert_eq!(tree.hit_test(Point::new(10.0, 10.0)), None);
    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: visual_center,
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(*observed_pointer.borrow(), Some(Point::new(10.0, 5.0)));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::Wheel {
            pos: visual_center,
            delta: Point::new(0.0, 10.0),
        }),
        EventResult::Handled
    );
    assert_eq!(
        *observed_wheel.borrow(),
        Some((Point::new(10.0, 5.0), Point::new(0.0, 5.0)))
    );

    let semantic = tree.semantic_snapshot_body();
    assert_eq!(semantic.nodes[0].frame, Rect::new(30.0, 15.0, 40.0, 20.0));
    assert_eq!(
        semantic.nodes[0].visible_bounds,
        Some(Rect::new(30.0, 15.0, 40.0, 20.0))
    );
}

#[test]
fn zero_scale_subtree_is_not_hittable() {
    let mut tree = ViewAdapter::build(label("collapsed").width(20.0).height(10.0).scale(0.0));
    let root = tree.root_id().expect("root");
    tree.get_mut(root)
        .expect("root node")
        .set_frame(Rect::new(10.0, 10.0, 20.0, 10.0));

    assert_eq!(tree.hit_test(Point::new(20.0, 15.0)), None);
}

#[test]
fn mount_transition_advances_visual_bounds_and_opacity_without_reconcile() {
    let mut tree = ViewAdapter::build(
        label("enter")
            .width(20.0)
            .height(10.0)
            .enter_animation(AnimationConfig::slide_in(Placement::Right, 1.0)),
    );
    let root = tree.root_id().expect("root");
    tree.get_mut(root)
        .expect("root node")
        .set_frame(Rect::new(10.0, 10.0, 20.0, 10.0));
    tree.reconcile_lifecycle_after_layout();

    assert!(tree.get(root).unwrap().view_transition_active());
    assert_eq!(tree.get(root).unwrap().view_transition_opacity(), 0.0);
    assert_eq!(
        tree.node_visual_rect(root, Rect::new(10.0, 10.0, 20.0, 10.0)),
        Some(Rect::new(34.0, 10.0, 20.0, 10.0))
    );

    assert_eq!(tree.update_animation_nodes([root], 0.5), vec![(root, true)]);
    let halfway_opacity = tree.get(root).unwrap().view_transition_opacity();
    assert!(halfway_opacity > 0.0 && halfway_opacity < 1.0);
    let halfway = tree
        .node_visual_rect(root, Rect::new(10.0, 10.0, 20.0, 10.0))
        .expect("halfway visual bounds");
    assert!(halfway.x > 10.0 && halfway.x < 34.0);

    assert_eq!(
        tree.update_animation_nodes([root], 0.5),
        vec![(root, false)]
    );
    assert!(!tree.get(root).unwrap().view_transition_active());
    assert_eq!(tree.get(root).unwrap().view_transition_opacity(), 1.0);
    assert_eq!(
        tree.node_visual_rect(root, Rect::new(10.0, 10.0, 20.0, 10.0)),
        Some(Rect::new(10.0, 10.0, 20.0, 10.0))
    );
}

#[test]
fn zero_duration_mount_transition_is_immediately_at_rest() {
    let tree = ViewAdapter::build(label("instant").enter_animation(AnimationConfig::zoom_in(0.0)));
    let root = tree.root_id().expect("root");

    assert!(!tree.get(root).unwrap().view_transition_active());
    assert_eq!(tree.get(root).unwrap().view_transition_opacity(), 1.0);
    assert!(tree
        .get(root)
        .unwrap()
        .visual_transform_matrix()
        .is_identity());
}

#[test]
#[should_panic(expected = "enter_animation requires fade_in, slide_in, or zoom_in")]
fn mount_transition_rejects_exit_presets() {
    let _ = label("invalid").enter_animation(AnimationConfig::fade_out(0.2));
}

#[test]
#[should_panic(expected = "leave_animation requires fade_out, slide_out, or zoom_out")]
fn leave_transition_rejects_enter_presets() {
    let _ = label("invalid").leave_animation(AnimationConfig::fade_in(0.2));
}

#[test]
fn mount_transition_does_not_restart_when_the_keyed_node_reconciles() {
    let view = || {
        label("stable")
            .key("stable")
            .enter_animation(AnimationConfig::fade_in(1.0))
    };
    let mut tree = ViewAdapter::build(view());
    tree.layout();
    let root = tree.root_id().expect("root");
    assert_eq!(
        tree.update_animation_nodes([root], 0.25),
        vec![(root, true)]
    );
    let before = tree.get(root).unwrap().view_transition_opacity();

    ViewAdapter::reconcile(&mut tree, view());

    assert_eq!(tree.root_id(), Some(root));
    assert_eq!(tree.get(root).unwrap().view_transition_opacity(), before);
    assert_eq!(
        tree.update_animation_nodes([root], 0.75),
        vec![(root, false)]
    );
}

#[test]
fn keyed_removal_leaves_visually_then_removes_and_requests_layout() {
    let item = || {
        label("leaving")
            .key("item")
            .width(40.0)
            .height(20.0)
            .leave_animation(AnimationConfig::fade_out(1.0))
    };
    let mut tree = ViewAdapter::build(column(vec![item()]));
    tree.layout();
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];
    let frame = tree.get(child).unwrap().frame();
    let center = Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5);
    assert_eq!(tree.hit_test(center), Some(child));

    ViewAdapter::reconcile(&mut tree, column(Vec::<crate::ui::view::ViewNode>::new()));

    assert!(tree.get(child).unwrap().pending_removal());
    assert_eq!(tree.get(root).unwrap().children(), &[child]);
    assert_ne!(tree.hit_test(center), Some(child));
    assert!(tree
        .semantic_snapshot_body()
        .nodes
        .iter()
        .all(|node| node.id != child));
    assert_eq!(tree.active_view_transition_ids(), vec![child]);

    assert_eq!(
        tree.update_animation_nodes([child], 0.5),
        vec![(child, true)]
    );
    let opacity = tree.get(child).unwrap().view_transition_opacity();
    assert!(opacity > 0.0 && opacity < 1.0);
    assert_eq!(
        tree.update_animation_nodes([child], 0.5),
        vec![(child, false)]
    );
    assert!(tree.get(child).is_none());
    assert!(tree.get(root).unwrap().children().is_empty());
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .has_layout());
}

#[test]
fn leaving_subtree_releases_focus_immediately() {
    let item = || {
        label("focusable")
            .key("item")
            .focusable(true)
            .leave_animation(AnimationConfig::fade_out(1.0))
    };
    let mut tree = ViewAdapter::build(column(vec![item()]));
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];
    tree.set_focus(Some(child));
    assert_eq!(tree.managers().focus.focused_component(), Some(child));

    ViewAdapter::reconcile(&mut tree, column(Vec::<crate::ui::view::ViewNode>::new()));

    assert_eq!(tree.managers().focus.focused_component(), None);
    assert!(!tree.collect_focusable().contains(&child));
    assert!(tree.get(child).is_some());
}

#[test]
fn leaving_subtree_releases_timer_and_overlay_work_immediately() {
    use crate::ui::widgets::Tooltip;

    let item = || {
        crate::ui::view::ViewNode::leaf(Tooltip::new("Help").delay_ms(300).timer_id(42))
            .key("item")
            .leave_animation(AnimationConfig::fade_out(1.0))
    };
    let mut tree = ViewAdapter::build(column(vec![item()]));
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.dispatch_to(child, &SystemEvent::PointerEnter),
        EventResult::Handled
    );
    let timer = tree
        .active_timers()
        .into_iter()
        .find(|(id, _)| *id == crate::ui::core::widget::WidgetTree::timer_work_key(child, 42))
        .expect("tooltip timer");
    assert_eq!(tree.dispatch_timer_work(timer.0), EventResult::Handled);
    assert!(tree
        .overlay_stack()
        .iter()
        .any(|entry| entry.owner() == child));

    ViewAdapter::reconcile(&mut tree, column(Vec::<crate::ui::view::ViewNode>::new()));

    assert!(tree.active_timers().is_empty());
    assert_eq!(
        tree.dispatch_to(child, &SystemEvent::Timer { id: 42 }),
        EventResult::NotHandled
    );
    assert!(tree
        .overlay_stack()
        .iter()
        .all(|entry| entry.owner() != child));
    assert!(tree.get(child).is_some());

    ViewAdapter::reconcile(&mut tree, column(vec![item()]));

    assert_eq!(tree.get(root).unwrap().children(), &[child]);
    assert!(tree
        .overlay_stack()
        .iter()
        .any(|entry| entry.owner() == child));
}

#[test]
fn keyed_reappearance_cancels_leave_and_reuses_the_same_node() {
    let item = || {
        label("stable")
            .key("item")
            .leave_animation(AnimationConfig::zoom_out(1.0))
    };
    let mut tree = ViewAdapter::build(column(vec![item()]));
    tree.layout();
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];

    ViewAdapter::reconcile(&mut tree, column(Vec::<crate::ui::view::ViewNode>::new()));
    assert_eq!(
        tree.update_animation_nodes([child], 0.25),
        vec![(child, true)]
    );
    assert!(tree.get(child).unwrap().view_transition_opacity() < 1.0);

    ViewAdapter::reconcile(&mut tree, column(vec![item()]));

    assert_eq!(tree.get(root).unwrap().children(), &[child]);
    assert!(!tree.get(child).unwrap().pending_removal());
    assert!(!tree.get(child).unwrap().view_transition_active());
    assert_eq!(tree.get(child).unwrap().view_transition_opacity(), 1.0);
}

#[test]
fn leave_started_during_enter_preserves_the_current_visual_state() {
    let item = || {
        label("handoff")
            .key("item")
            .width(40.0)
            .height(20.0)
            .enter_animation(AnimationConfig::slide_in(Placement::Right, 1.0))
            .leave_animation(AnimationConfig::fade_out(1.0))
    };
    let mut tree = ViewAdapter::build(column(vec![item()]));
    tree.layout();
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];
    assert_eq!(
        tree.update_animation_nodes([child], 0.25),
        vec![(child, true)]
    );
    let frame = tree.get(child).unwrap().frame();
    let before_rect = tree.node_visual_rect(child, frame);
    let before_opacity = tree.get(child).unwrap().view_transition_opacity();

    ViewAdapter::reconcile(&mut tree, column(Vec::<crate::ui::view::ViewNode>::new()));

    assert_eq!(tree.node_visual_rect(child, frame), before_rect);
    assert_eq!(
        tree.get(child).unwrap().view_transition_opacity(),
        before_opacity
    );
}

#[test]
fn zero_duration_leave_removes_the_keyed_node_immediately() {
    let mut tree = ViewAdapter::build(column(vec![label("instant")
        .key("item")
        .leave_animation(AnimationConfig::fade_out(0.0))]));
    let root = tree.root_id().expect("root");
    let child = tree.get(root).unwrap().children()[0];

    ViewAdapter::reconcile(&mut tree, column(Vec::<crate::ui::view::ViewNode>::new()));

    assert!(tree.get(child).is_none());
    assert!(tree.get(root).unwrap().children().is_empty());
}

#[test]
fn transform_reconcile_damages_old_and_new_subtree_bounds_without_layout() {
    let mut tree = ViewAdapter::build(label("target").width(20.0).height(10.0));
    let root = tree.root_id().expect("root");
    tree.get_mut(root)
        .expect("root node")
        .set_frame(Rect::new(10.0, 10.0, 20.0, 10.0));
    tree.reset_invalidation();
    let version = tree.tree_version();

    ViewAdapter::reconcile(
        &mut tree,
        label("target")
            .width(20.0)
            .height(10.0)
            .offset(Point::new(30.0, 0.0)),
    );

    assert!(
        tree.tree_version() > version,
        "LayerTree must rebuild metadata"
    );
    assert_eq!(
        tree.dirty_region().bounds(),
        Rect::new(10.0, 10.0, 50.0, 10.0)
    );
    assert!(!tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .has_layout());
}

#[test]
fn widget_tree_scene_bridge_renders_the_transformed_canvas_subtree() {
    let mut tree = ViewAdapter::build_nodes(
        canvas(10.0, 10.0, |frame, ctx| {
            ctx.fill_rect(frame, Color::red(), None);
        })
        .offset(Point::new(10.0, 5.0))
        .scale(2.0),
    );
    let root = tree.root_id().expect("root");
    tree.get_mut(root)
        .expect("root node")
        .set_frame(Rect::new(20.0, 20.0, 10.0, 10.0));
    tree.invalidate_paint(root);

    let mut layers = LayerTree::new();
    layers.build(&tree, false);
    let mut engine = SoftwareEngine::new();
    engine.initialize(64, 64).expect("software engine init");
    let tokens = DesignTokens::antd_light();
    let theme = ThemeSnapshot::new(&tokens);
    let fonts = FontService::new();
    let images = ImageService::new();
    layers
        .render(
            &mut engine,
            &tree,
            &DirtyRegion::full(),
            &theme,
            FontHandle::default(),
            &fonts,
            &images,
            false,
            None,
            None,
        )
        .expect("transformed WidgetTree render");

    let pixels = engine.canvas_2d().pixels_mut();
    let at = |x: usize, y: usize| pixels[y * 64 + x];
    assert_eq!(
        at(20, 20),
        0,
        "layout location must not retain stale pixels"
    );
    assert_ne!(at(25, 20), 0, "visual top-left must be painted");
    assert_ne!(at(44, 39), 0, "visual lower-right must be painted");
    assert_eq!(at(45, 39), 0, "visual width must remain bounded");
}
