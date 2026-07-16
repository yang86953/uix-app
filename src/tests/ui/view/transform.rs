use std::cell::RefCell;
use std::rc::Rc;

use crate::draw::compositor::LayerTree;
use crate::tests::common::*;
use crate::ui::core::widget::WidgetCore;
use crate::ui::view::{canvas, label, ViewAdapter};
use crate::ui::{EventResult, SystemEvent};

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
