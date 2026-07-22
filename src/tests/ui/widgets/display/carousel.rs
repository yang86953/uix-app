use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::painting::PaintPass;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::view::{StyleExt, ViewAdapter, ViewNode};
use crate::ui::widgets::display::carousel::{Carousel, CarouselEffect};
use crate::ui::widgets::{Button, Container, Label};
use crate::ui::LayoutChild;

fn layout_carousel(carousel: &Carousel, count: usize) -> Vec<(ComponentId, Rect)> {
    let children = (0..count)
        .map(|index| LayoutChild::new(ComponentId::new(index), Size::new(80.0, 40.0)))
        .collect::<Vec<_>>();
    carousel.layout_children(
        Rect::new(10.0, 20.0, 300.0, 200.0),
        &children,
        &WidgetTree::new(),
    )
}

#[test]
fn carousel_layout_keeps_only_the_current_slide_visible() {
    let carousel = Carousel::new();

    let placements = layout_carousel(&carousel, 3);

    assert_eq!(carousel.slide_count(), 3);
    assert_eq!(WidgetComponent::tab_index(&carousel), 1);
    assert_eq!(placements[0].1, Rect::new(10.0, 20.0, 300.0, 200.0));
    assert_eq!(placements[1].1, Rect::new(10.0, 20.0, 0.0, 0.0));
    assert_eq!(placements[2].1, Rect::new(10.0, 20.0, 0.0, 0.0));
}

#[test]
fn carousel_keyboard_navigation_wraps_and_emits_index() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);
    let right = SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        carousel.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(carousel.on_event(&right), EventResult::Handled);
    assert_eq!(carousel.current_index(), 1);
    assert_eq!(
        carousel
            .semantic_event(ComponentId::new(9), &right)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("1".to_string())
    );

    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });
    assert_eq!(carousel.current_index(), 2);
    carousel.on_event(&right);
    assert_eq!(carousel.current_index(), 0);
}

#[test]
fn carousel_pointer_navigation_uses_node_local_coordinates() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);

    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(150.0, 185.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 1);
}

#[test]
fn carousel_snapshot_and_accessibility_expose_runtime_slide() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);
    carousel.on_event(&SystemEvent::FocusIn);
    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    let fields = carousel.snapshot_fields();

    assert_eq!(
        fields,
        SnapshotFields::Carousel {
            show_dots: true,
            show_arrows: true,
            fixed_width: None,
            fixed_height: None,
            current: 2,
            slide_count: 3,
        }
    );
    let accessibility = fields.accessibility();
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("Slide 3 of 3")
    );
    assert_eq!(accessibility.state.value_now, Some(3.0));
    assert_eq!(accessibility.state.value_max, Some(3.0));
}

#[test]
fn carousel_size_sets_the_preferred_viewport_and_disables_flex_growth() {
    let carousel = Carousel::new().size(140.0, 88.0);

    assert_eq!(
        carousel.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(140.0, 88.0)
    );
    assert_eq!(carousel.flex_grow(), 0.0);
    assert_eq!(
        carousel.snapshot_fields(),
        SnapshotFields::Carousel {
            show_dots: true,
            show_arrows: true,
            fixed_width: Some(140.0),
            fixed_height: Some(88.0),
            current: 0,
            slide_count: 0,
        }
    );
}

#[test]
fn carousel_hides_inactive_slide_subtrees_from_focus_and_hit_testing() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Carousel::new()));
    let first = tree.add_child(root, Box::new(Container::new()));
    let first_button = tree.add_child(first, Box::new(Button::new("First")));
    let second = tree.add_child(root, Box::new(Container::new()));
    let second_button = tree.add_child(second, Box::new(Button::new("Second")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));

    tree.layout();

    assert!(tree.get(first).unwrap().visible());
    assert!(!tree.get(second).unwrap().visible());
    assert!(tree.is_effectively_visible(first_button));
    assert!(!tree.is_effectively_visible(second_button));
    assert!(tree.collect_focusable().contains(&first_button));
    assert!(!tree.collect_focusable().contains(&second_button));

    tree.set_focus(Some(root));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    tree.layout();

    assert!(!tree.get(first).unwrap().visible());
    assert!(tree.get(second).unwrap().visible());
    assert!(!tree.is_effectively_visible(first_button));
    assert!(tree.is_effectively_visible(second_button));
    assert!(!tree.collect_focusable().contains(&first_button));
    assert!(tree.collect_focusable().contains(&second_button));
}

#[test]
fn carousel_controls_capture_pointer_over_interactive_slides_and_take_focus() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Carousel::new()));
    let first = tree.add_child(root, Box::new(Button::new("First")));
    let _second = tree.add_child(root, Box::new(Button::new("Second")));
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();

    tree.set_focus(Some(first));
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled,
        "a focused slide control must retain its arrow-key behavior"
    );
    assert_eq!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Carousel>()
            .unwrap()
            .current_index(),
        0
    );

    assert_eq!(
        tree.dispatch_event(&SystemEvent::PointerDown {
            pos: Point::new(295.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(tree.managers().focus.focused_component(), Some(root));
    assert_eq!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Carousel>()
            .unwrap()
            .current_index(),
        1
    );
}

#[test]
fn carousel_narrow_controls_keep_both_arrows_and_dots_reachable() {
    let mut carousel = Carousel::new();
    let children = (0..20)
        .map(|index| LayoutChild::new(ComponentId::new(index), Size::new(10.0, 10.0)))
        .collect::<Vec<_>>();
    carousel.layout_children(
        Rect::new(0.0, 0.0, 24.0, 40.0),
        &children,
        &WidgetTree::new(),
    );

    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 19);
    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(23.0, 10.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 0);
    assert_eq!(
        carousel.on_event(&SystemEvent::PointerDown {
            pos: Point::new(12.5, 32.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(carousel.current_index(), 10);
}

#[test]
fn carousel_clamps_current_when_children_are_removed() {
    let mut carousel = Carousel::new();
    layout_carousel(&carousel, 3);
    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::End,
        mods: KeyMod::NONE,
    });

    let placements = layout_carousel(&carousel, 1);

    assert_eq!(carousel.current_index(), 0);
    assert_eq!(carousel.slide_count(), 1);
    assert_eq!(WidgetComponent::tab_index(&carousel), 0);
    assert_eq!(placements[0].1, Rect::new(10.0, 20.0, 300.0, 200.0));
}

#[test]
fn carousel_change_requests_narrow_layout_and_swaps_real_child_frames() {
    let mut tree = WidgetTree::new();
    let root = tree.set_root(Box::new(Carousel::new()));
    let first = tree.add_child(root, Box::new(Label::new("First")));
    let second = tree.add_child(root, Box::new(Label::new("Second")));
    tree.get_mut(root)
        .expect("carousel root")
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    assert!(
        tree.get(root).unwrap().is_focusable(),
        "child-dependent tab_index must refresh after Carousel layout"
    );
    assert!(tree.collect_focusable().contains(&root));
    tree.set_focus(Some(root));
    tree.invalidation().lock().unwrap().clear();

    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(tree
        .invalidation()
        .lock()
        .unwrap()
        .layout_roots()
        .contains(&root));

    tree.layout();
    assert_eq!(
        tree.get(first).unwrap().frame(),
        Rect::new(0.0, 0.0, 0.0, 0.0)
    );
    assert_eq!(
        tree.get(second).unwrap().frame(),
        Rect::new(0.0, 0.0, 300.0, 200.0)
    );
}

fn render_carousel_overlay(carousel: &Carousel, frame: Rect) -> String {
    let surface_size = (frame.w.ceil() as i32, frame.h.ceil() as i32);
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut display_list = crate::draw::painting::DisplayList::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            font,
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            surface_size.0,
            surface_size.1,
        );
        ctx.set_paint_pass(PaintPass::AfterChildren);
        ctx.with_recorder(&mut display_list, |ctx| {
            WidgetRender::render(carousel, frame, ctx, &tree);
        });
    }
    format!("{display_list:?}")
}

fn custom_arrow_carousel(changes: Rc<RefCell<Vec<String>>>, slide_count: usize) -> ViewNode {
    let changes_for_handler = changes.clone();
    ViewNode::new(
        Carousel::new().arrows(|previous, next| {
            crate::ui::view::row((
                crate::ui::view::button("previous")
                    .automation_id("carousel-previous")
                    .on_click_fn(move || previous()),
                crate::ui::view::button("next")
                    .automation_id("carousel-next")
                    .on_click_fn(move || next()),
            ))
        }),
        (0..slide_count)
            .map(|index| ViewNode::leaf(Label::new(format!("slide-{index}"))))
            .collect(),
    )
    .on_semantic(SemanticKind::Change, move |event| {
        if let Some(index) = event.text_payload() {
            changes_for_handler.borrow_mut().push(index.to_owned());
        }
    })
}

fn automation_id(tree: &WidgetTree, value: &str) -> ComponentId {
    tree.traverse()
        .iter()
        .copied()
        .find(|&id| {
            tree.get(id)
                .is_some_and(|node| node.automation_id() == Some(value))
        })
        .expect("automation id")
}

fn frame_center(tree: &WidgetTree, id: ComponentId) -> Point {
    let frame = tree.get(id).expect("widget frame").frame();
    Point::new(frame.x + frame.w * 0.5, frame.y + frame.h * 0.5)
}

fn click_tree(tree: &mut WidgetTree, pos: Point) {
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
}

#[test]
fn carousel_custom_arrows_materialize_as_a_real_child() {
    let tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Carousel::new().arrows(|_, _| crate::ui::view::label("custom arrows")),
    ));

    assert!(tree.find_by_type::<Label>().is_some());
}

#[test]
fn carousel_autoplay_wraps_and_manual_navigation_pauses_until_focus_leaves() {
    let interval = Duration::from_millis(750);
    let mut carousel = Carousel::new().autoplay(interval);
    layout_carousel(&carousel, 3);
    let (timer_id, delay) = EventHandler::active_timer(&carousel).expect("autoplay timer");
    assert_eq!(delay, interval);

    for expected in [1, 2, 0] {
        assert_eq!(
            carousel.on_event(&SystemEvent::Timer {
                id: timer_id as u32,
            }),
            EventResult::Handled
        );
        assert_eq!(carousel.current_index(), expected);
    }

    carousel.on_event(&SystemEvent::FocusIn);
    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(carousel.current_index(), 1);
    assert_eq!(EventHandler::active_timer(&carousel), None);
    carousel.on_event(&SystemEvent::Timer {
        id: timer_id as u32,
    });
    assert_eq!(carousel.current_index(), 1, "stale timer must stay paused");

    carousel.on_event(&SystemEvent::FocusOut);
    assert_eq!(
        EventHandler::active_timer(&carousel),
        Some((timer_id, interval))
    );
}

#[test]
fn carousel_autoplay_obeys_zero_single_slide_and_hover_boundaries() {
    let interval = Duration::from_secs(1);
    let zero = Carousel::new().autoplay(Duration::ZERO);
    layout_carousel(&zero, 2);
    assert_eq!(EventHandler::active_timer(&zero), None);

    let single = Carousel::new().autoplay(interval);
    layout_carousel(&single, 1);
    assert_eq!(EventHandler::active_timer(&single), None);

    let mut carousel = Carousel::new().autoplay(interval).pause_on_hover(true);
    layout_carousel(&carousel, 2);
    let (timer_id, _) = EventHandler::active_timer(&carousel).expect("autoplay timer");
    assert_eq!(
        carousel.on_event(&SystemEvent::PointerMove {
            pos: Point::new(100.0, 100.0),
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(EventHandler::active_timer(&carousel), None);
    carousel.on_event(&SystemEvent::Timer {
        id: timer_id as u32,
    });
    assert_eq!(carousel.current_index(), 0);

    carousel.on_event(&SystemEvent::PointerMove {
        pos: Point::new(400.0, 100.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(
        EventHandler::active_timer(&carousel),
        Some((timer_id, interval))
    );
}

#[test]
fn carousel_fade_animates_overlay_and_swaps_slide_at_midpoint() {
    let frame = Rect::new(10.0, 20.0, 300.0, 200.0);
    let mut carousel = Carousel::new()
        .show_dots(false)
        .show_arrows(false)
        .effect(CarouselEffect::Fade);
    layout_carousel(&carousel, 2);
    carousel.on_event(&SystemEvent::FocusIn);
    carousel.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(carousel.current_index(), 1);
    assert!(carousel.take_layout_request());

    let before_midpoint = layout_carousel(&carousel, 2);
    assert_eq!(before_midpoint[0].1, frame);
    assert_eq!(before_midpoint[1].1, Rect::new(10.0, 20.0, 0.0, 0.0));
    assert!(WidgetAnimation::update_animation(&mut carousel, 0.075));
    assert_eq!(WidgetAnimation::dirty_bounds(&carousel, frame), frame);
    assert!(
        render_carousel_overlay(&carousel, frame).contains("FillRect"),
        "fade must enter the overlay paint path"
    );

    assert!(WidgetAnimation::update_animation(&mut carousel, 0.075));
    assert!(carousel.take_layout_request());
    let after_midpoint = layout_carousel(&carousel, 2);
    assert_eq!(after_midpoint[0].1, Rect::new(10.0, 20.0, 0.0, 0.0));
    assert_eq!(after_midpoint[1].1, frame);

    assert!(!WidgetAnimation::update_animation(&mut carousel, 0.15));
    assert!(!render_carousel_overlay(&carousel, frame).contains("FillRect"));
}

#[test]
fn carousel_custom_arrows_append_to_slides_and_survive_reconcile() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let mut tree = ViewAdapter::build_nodes(custom_arrow_carousel(changes.clone(), 2));
    let root = tree.root_id().expect("carousel root");
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    assert_eq!(tree.get(root).unwrap().children().len(), 3);
    assert_eq!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Carousel>()
            .unwrap()
            .slide_count(),
        2
    );
    tree.invalidation().lock().unwrap().clear();

    let next = automation_id(&tree, "carousel-next");
    let next_center = frame_center(&tree, next);
    click_tree(&mut tree, next_center);
    assert_eq!(&*changes.borrow(), &["1".to_string()]);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap()
        .layout_roots()
        .contains(&root));
    tree.layout();

    ViewAdapter::reconcile_nodes(&mut tree, custom_arrow_carousel(changes.clone(), 2));
    assert_eq!(tree.root_id(), Some(root));
    assert_eq!(tree.get(root).unwrap().children().len(), 3);
    assert_eq!(
        tree.get(root)
            .unwrap()
            .component()
            .as_any()
            .downcast_ref::<Carousel>()
            .unwrap()
            .current_index(),
        1
    );

    let previous = automation_id(&tree, "carousel-previous");
    tree.set_focus(Some(previous));
    tree.invalidation().lock().unwrap().clear();
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        tree.dispatch_event(&SystemEvent::KeyUp {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(&*changes.borrow(), &["1".to_string(), "0".to_string()]);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap()
        .layout_roots()
        .contains(&root));
}

#[test]
fn carousel_custom_arrow_noop_does_not_request_layout_or_emit_change() {
    let changes = Rc::new(RefCell::new(Vec::new()));
    let mut tree = ViewAdapter::build_nodes(custom_arrow_carousel(changes.clone(), 1));
    let root = tree.root_id().expect("carousel root");
    tree.get_mut(root)
        .unwrap()
        .set_frame(Rect::new(0.0, 0.0, 300.0, 200.0));
    tree.layout();
    tree.invalidation().lock().unwrap().clear();

    let next = automation_id(&tree, "carousel-next");
    let next_center = frame_center(&tree, next);
    click_tree(&mut tree, next_center);

    assert!(changes.borrow().is_empty());
    assert!(tree
        .invalidation()
        .lock()
        .unwrap()
        .layout_roots()
        .is_empty());
}
