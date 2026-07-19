use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Slider;
use crate::ui::{with_config, ComponentConfig};

fn render_slider(slider: &Slider, frame: Rect) -> Vec<u32> {
    let mut canvas = CpuCanvas2D::new(PixelSurface::new(240, 64));
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    {
        let mut ctx = PaintContext::new_for_test(
            &mut canvas,
            FontHandle::default(),
            &fonts,
            &images,
            &tokens,
            96.0,
            1.0,
            Orientation::YDown,
            240,
            64,
        );
        WidgetRender::render(slider, frame, &mut ctx, &tree);
    }
    canvas.surface().pixels().to_vec()
}

#[test]
fn bound_slider_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new(4.0);
    let mut slider = Slider::new(0.0..=10.0).step(2.0).value(&value);

    assert_eq!(slider.current_value(), 4.0);
    assert_eq!(
        slider.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(slider.current_value(), 6.0);
    assert_eq!(value.get(), 6.0);

    value.set(20.0);
    slider.sync_from(Slider::new(0.0..=10.0).step(2.0).value(&value));
    assert_eq!(slider.current_value(), 10.0);

    let _ = slider.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_value(), 8.0);
    assert_eq!(value.get(), 8.0);
}

#[test]
fn uncontrolled_slider_keeps_runtime_value_across_reconcile() {
    let mut slider = Slider::new(1.0..=10.0).step(2.0).default_value(1.0);
    let _ = slider.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_value(), 3.0);

    slider.sync_from(Slider::new(-10.0..=10.0).step(0.5).default_value(-5.0));
    assert_eq!(slider.current_value(), 3.0);
}

#[test]
fn marks_are_sorted_deduplicated_bounded_and_preserved_in_snapshot() {
    let mut slider = Slider::new(0.0..=100.0).default_value(25.0).marks(vec![
        (100.0, "End"),
        (f64::NAN, "NaN"),
        (-1.0, "Before"),
        (50.0, "Old middle"),
        (0.0, "Start"),
        (50.0, "Middle"),
    ]);
    assert_eq!(
        slider.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(200.0, 50.0)
    );
    assert!(matches!(
        slider.snapshot_fields(),
        SnapshotFields::Slider { marks, .. }
            if marks == [
                (0.0, "Start".to_owned()),
                (50.0, "Middle".to_owned()),
                (100.0, "End".to_owned()),
            ]
    ));

    slider.sync_from(
        Slider::new(0.0..=100.0)
            .default_value(80.0)
            .marks(vec![(25.0, "Quarter")]),
    );
    assert_eq!(slider.current_value(), 25.0);
    assert!(matches!(
        slider.snapshot_fields(),
        SnapshotFields::Slider { marks, .. }
            if marks == [(25.0, "Quarter".to_owned())]
    ));
}

#[test]
fn marks_paint_a_tick_outside_the_plain_track_without_expanding_hit_geometry() {
    let frame = Rect::new(0.0, 0.0, 200.0, 50.0);
    let plain = Slider::new(0.0..=100.0).default_value(50.0);
    let plain_pixels = render_slider(&plain, frame);
    let marked = Slider::new(0.0..=100.0)
        .default_value(50.0)
        .marks(vec![(25.0, "Quarter")]);
    let marked_pixels = render_slider(&marked, frame);

    let tick_x = 53usize;
    let tick_top = 13usize;
    assert_ne!(
        plain_pixels[tick_top * 240 + tick_x],
        marked_pixels[tick_top * 240 + tick_x],
        "mark dot must extend beyond the plain medium track"
    );

    let mut marked = marked;
    assert_eq!(
        marked.on_event(&SystemEvent::PointerDown {
            pos: Point::new(53.0, 40.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled,
        "the label band is presentation-only"
    );
}

#[test]
fn external_slider_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new(4.0);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Slider::new(0.0..=10.0).value(&value))
    }));
    let root = tree.root_id().expect("slider root");
    tree.reset_invalidation();

    value.set(7.0);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(Slider::new(0.0..=10.0).value(&value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let slider = tree
        .get(root)
        .expect("slider node")
        .component()
        .as_any()
        .downcast_ref::<Slider>()
        .expect("Slider component");
    assert_eq!(slider.current_value(), 7.0);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_drive_slider_track_geometry() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut slider = with_config(&large, || Slider::new(0.0..=100.0).step(0.0));
    assert_eq!(slider.measure(max).h, 40.0);

    render_slider(&slider, Rect::new(24.0, 12.0, 200.0, 40.0));
    let _ = slider.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 20.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let expected = (10.0 - 7.5) / (200.0 - 15.0) * 100.0;
    assert!((slider.current_value() - expected).abs() < 0.000_001);

    let small = with_config(&large, || Slider::new(0.0..=1.0).size(ControlSize::Small));
    assert_eq!(small.measure(max).h, 24.0);
}

#[test]
fn keyboard_step_aligns_to_range_start_without_decimal_tail() {
    let mut off_grid = Slider::new(1.0..=10.0).step(2.0).default_value(2.0);
    let _ = off_grid.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(off_grid.current_value(), 3.0);
    let _ = off_grid.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(off_grid.current_value(), 1.0);

    let mut decimal = Slider::new(0.0..=1.0).step(0.1);
    for _ in 0..3 {
        let _ = decimal.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        });
    }
    assert_eq!(decimal.current_value(), 0.3);
}

#[test]
fn pointer_step_snaps_without_decimal_tail() {
    let mut slider = Slider::new(0.0..=1.0).step(0.1);
    let _ = render_slider(&slider, Rect::new(0.0, 0.0, 200.0, 32.0));

    let result = slider.on_event(&SystemEvent::PointerDown {
        pos: Point::new(62.4, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(result, EventResult::Handled);
    assert_eq!(slider.current_value(), 0.3);
}

#[test]
fn constrained_slider_paint_stays_inside_actual_frame() {
    let mut slider = Slider::new(0.0..=100.0)
        .size(ControlSize::Large)
        .marks(vec![(0.0, "Start"), (100.0, "End")])
        .default_value(50.0);
    let _ = slider.on_event(&SystemEvent::FocusIn);
    let frame = Rect::new(0.0, 0.0, 8.0, 10.0);
    let pixels = render_slider(&slider, frame);

    for y in 0..64 {
        for x in 0..240 {
            if x >= frame.w as usize || y >= frame.h as usize {
                assert_eq!(pixels[y * 240 + x], 0, "paint leaked at ({x}, {y})");
            }
        }
    }
}

#[test]
fn pointer_down_rejects_points_outside_the_rendered_frame() {
    let mut slider = Slider::new(0.0..=100.0).default_value(25.0);
    let _ = render_slider(&slider, Rect::new(0.0, 0.0, 80.0, 10.0));

    let result = slider.on_event(&SystemEvent::PointerDown {
        pos: Point::new(10.0, 12.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(result, EventResult::NotHandled);
    assert_eq!(slider.current_value(), 25.0);
}

#[test]
fn captured_drag_continues_after_pointer_leave_until_release() {
    let mut slider = Slider::new(0.0..=100.0);
    let _ = render_slider(&slider, Rect::new(0.0, 0.0, 200.0, 32.0));
    let _ = slider.on_event(&SystemEvent::PointerDown {
        pos: Point::new(6.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = slider.on_event(&SystemEvent::PointerLeave);
    let _ = slider.on_event(&SystemEvent::PointerMove {
        pos: Point::new(194.0, 16.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_value(), 100.0);

    let _ = slider.on_event(&SystemEvent::PointerUp {
        pos: Point::new(194.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = slider.on_event(&SystemEvent::PointerMove {
        pos: Point::new(6.0, 16.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_value(), 100.0);
}
