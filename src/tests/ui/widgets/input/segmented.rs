use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::engine::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Segmented;
use crate::ui::{with_config, ComponentConfig};

fn render_segmented(segmented: &Segmented, frame: Rect, surface_size: (i32, i32)) -> Vec<u32> {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();

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
        WidgetRender::render(segmented, frame, &mut ctx, &tree);
    }
    canvas.surface().pixels().to_vec()
}

#[test]
fn segmented_cjk_width_uses_unicode_metrics_instead_of_utf8_bytes() {
    let options = ["每日", "每周", "每月", "每年"];
    let segmented = Segmented::new(options);
    let measured = segmented.measure(Constraints::unconstrained());
    let expected = options
        .iter()
        .map(|option| {
            crate::draw::font::text_backend::estimate_text_metrics(option, f32::INFINITY, 13.0)
                .max_line_width
                + 24.0
        })
        .sum::<f32>();

    assert!((measured.w - expected).abs() < 0.01, "{measured:?}");
}

#[test]
fn constrained_segmented_clips_paint_and_rejects_pointer_below_rendered_height() {
    let mut segmented = Segmented::new(["Very long first", "Very long second"])
        .default_selected(1)
        .size(ControlSize::Large);
    let _ = segmented.on_event(&SystemEvent::FocusIn);
    let pixels = render_segmented(&segmented, Rect::new(0.0, 0.0, 120.0, 20.0), (320, 64));

    assert!(
        (0..24).all(|y| (121..320).all(|x| pixels[y * 320 + x] == 0)),
        "Segmented paint must stay inside the rendered control width"
    );
    assert_eq!(
        segmented.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 22.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(segmented.current_index(), Some(1));
}

#[test]
fn expanded_segmented_distributes_the_full_frame_to_clickable_segments() {
    let mut segmented = Segmented::new(["A", "B"]);
    let _ = render_segmented(&segmented, Rect::new(0.0, 0.0, 200.0, 32.0), (240, 64));

    assert_eq!(
        segmented.on_event(&SystemEvent::PointerDown {
            pos: Point::new(175.0, 16.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(segmented.current_index(), Some(1));
}

#[test]
fn segmented_right_and_bottom_edges_are_half_open() {
    let mut segmented = Segmented::new([
        "A first segment that is much wider than the constrained frame",
        "Second",
    ]);
    let _ = render_segmented(&segmented, Rect::new(0.0, 0.0, 120.0, 20.0), (180, 48));

    for pos in [Point::new(120.0, 10.0), Point::new(10.0, 20.0)] {
        assert_eq!(
            segmented.on_event(&SystemEvent::PointerDown {
                pos,
                button: MouseButton::Left,
                mods: KeyMod::NONE,
            }),
            EventResult::NotHandled,
            "right and bottom edges are outside the control: {pos:?}"
        );
    }
    assert_eq!(segmented.current_index(), Some(0));
}

#[test]
fn segmented_pointer_hover_only_handles_visible_changes() {
    let mut segmented = Segmented::new(["A", "B"]);
    let _ = render_segmented(&segmented, Rect::new(0.0, 0.0, 200.0, 32.0), (240, 64));
    let first = SystemEvent::PointerMove {
        pos: Point::new(25.0, 16.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(segmented.on_event(&first), EventResult::Handled);
    assert_eq!(segmented.on_event(&first), EventResult::NotHandled);

    let outside = SystemEvent::PointerMove {
        pos: Point::new(220.0, 16.0),
        mods: KeyMod::NONE,
    };
    assert_eq!(segmented.on_event(&outside), EventResult::Handled);
    assert_eq!(segmented.on_event(&outside), EventResult::NotHandled);
    assert_eq!(
        segmented.on_event(&SystemEvent::PointerLeave),
        EventResult::NotHandled
    );
}

#[test]
fn segmented_keyboard_wraps_and_skips_disabled_options() {
    let mut segmented = Segmented::new(["Day", "Week", "Month"])
        .default_selected(2)
        .disable_option(1);

    let _ = segmented.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(segmented.current_index(), Some(0));

    let _ = segmented.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(segmented.current_index(), Some(2));
}

#[test]
fn disabled_segmented_keeps_the_selected_thumb_visible() {
    let segmented = Segmented::new(["Alpha", "Beta"])
        .default_selected(1)
        .disabled(true);
    let size = segmented.measure(Constraints::unconstrained());
    let pixels = render_segmented(&segmented, Rect::new(0.0, 0.0, size.w, size.h), (180, 64));
    let first_background = pixels[4 * 180 + 4];
    let selected_background = pixels[4 * 180 + size.w.mul_add(0.5, 4.0) as usize];

    assert_ne!(
        selected_background, first_background,
        "disabled selection must remain visually distinguishable"
    );
}

#[test]
fn bound_segmented_writes_keyboard_changes_and_skips_disabled_options() {
    let value = State::new("Day".to_string());
    let mut segmented = Segmented::new(["Day", "Week", "Month"])
        .value(&value)
        .disable_option(1);

    assert_eq!(segmented.current_value().as_deref(), Some("Day"));
    assert_eq!(
        segmented.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(segmented.current_value().as_deref(), Some("Month"));
    assert_eq!(segmented.current_index(), Some(2));
    assert_eq!(value.get(), "Month");

    value.set("Day".to_string());
    segmented.sync_from(
        Segmented::new(["Day", "Week", "Month"])
            .value(&value)
            .disable_option(1),
    );
    assert_eq!(segmented.current_index(), Some(0));
}

#[test]
fn unmatched_controlled_value_moves_to_first_enabled_option() {
    let value = State::new("Unknown".to_string());
    let mut segmented = Segmented::new(["Day", "Week", "Month"])
        .value(&value)
        .disable_option(0);
    assert_eq!(segmented.current_value(), None);

    let _ = segmented.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(segmented.current_value().as_deref(), Some("Week"));
    assert_eq!(value.get(), "Week");
}

#[test]
fn uncontrolled_segmented_keeps_runtime_selection_across_reconcile() {
    let mut segmented = Segmented::new(["Day", "Week", "Month"]).default_selected(1);
    let _ = segmented.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(segmented.current_value().as_deref(), Some("Month"));

    segmented.sync_from(Segmented::new(["Day", "Week", "Month", "Year"]));
    assert_eq!(segmented.current_value().as_deref(), Some("Month"));
}

#[test]
fn uncontrolled_segmented_preserves_runtime_value_when_options_reorder() {
    let mut segmented = Segmented::new(["Day", "Week", "Month"]).default_selected(1);

    segmented.sync_from(Segmented::new(["Month", "Day", "Week"]));

    assert_eq!(segmented.current_value().as_deref(), Some("Week"));
    assert_eq!(segmented.current_index(), Some(2));
}

#[test]
fn external_segmented_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new("Day".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Segmented::new(["Day", "Week"]).value(&value))
    }));
    let root = tree.root_id().expect("segmented root");
    tree.reset_invalidation();

    value.set("Week".to_string());
    assert!(tree.take_reconcile_requested());
    let next =
        ViewAdapter::capture_root(|| ViewNode::leaf(Segmented::new(["Day", "Week"]).value(&value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let segmented = tree
        .get(root)
        .expect("segmented node")
        .component()
        .as_any()
        .downcast_ref::<Segmented>()
        .expect("Segmented component");
    assert_eq!(segmented.current_value().as_deref(), Some("Week"));
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_drive_segmented_layout_and_hit_widths() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut segmented = with_config(&large, || Segmented::new(["A", "B"]).default_selected(1));

    assert_eq!(segmented.measure(max).h, 40.0);
    assert_eq!(
        segmented.on_event(&SystemEvent::PointerDown {
            pos: Point::new(37.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(segmented.current_index(), Some(0));

    let small = with_config(&large, || {
        Segmented::new(["A", "B"]).size(ControlSize::Small)
    });
    assert_eq!(small.measure(max).h, 24.0);
}

#[test]
fn segmented_is_keyboard_focusable_and_moves_after_focus() {
    let mut segmented = Segmented::new(["Day", "Week"]);
    assert_eq!(WidgetComponent::tab_index(&segmented), 1);
    assert_eq!(
        segmented.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        segmented.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(segmented.current_value().as_deref(), Some("Week"));
    assert_eq!(
        segmented.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
}
