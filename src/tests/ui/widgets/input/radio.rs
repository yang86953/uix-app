use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Radio;
use crate::ui::{with_config, ComponentConfig};

fn assert_size_close(actual: Size, expected: Size) {
    assert!(
        (actual.w - expected.w).abs() < 0.001 && (actual.h - expected.h).abs() < 0.001,
        "expected {expected:?}, got {actual:?}"
    );
}

fn render_radio_pixels(radio: &Radio, frame: Rect) -> (Vec<u32>, usize) {
    const SURFACE_WIDTH: i32 = 240;
    const SURFACE_HEIGHT: i32 = 64;

    let mut canvas = CpuCanvas2D::new(PixelSurface::new(SURFACE_WIDTH, SURFACE_HEIGHT));
    let fonts = FontService::new();
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();
    let mut ctx = PaintContext::new_for_test(
        &mut canvas,
        FontHandle::default(),
        &fonts,
        &images,
        &tokens,
        96.0,
        1.0,
        Orientation::YDown,
        SURFACE_WIDTH,
        SURFACE_HEIGHT,
    );
    WidgetRender::render(radio, frame, &mut ctx, &tree);
    drop(ctx);
    (canvas.surface().pixels().to_vec(), SURFACE_WIDTH as usize)
}

fn pixel_at(pixels: &[u32], stride: usize, x: usize, y: usize) -> u32 {
    pixels[y * stride + x]
}

#[test]
fn bound_radio_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new("Medium".to_string());
    let mut radio = Radio::group("size", ["Small", "Medium", "Large"], &value);

    assert_eq!(radio.current_value().as_deref(), Some("Medium"));
    assert_eq!(radio.current_index(), Some(1));
    assert_eq!(
        radio.snapshot_fields().accessibility().name.as_deref(),
        Some("size")
    );
    assert_eq!(
        radio.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(radio.current_value().as_deref(), Some("Large"));
    assert_eq!(value.get(), "Large");

    value.set("Small".to_string());
    radio.sync_from(Radio::group("size", ["Small", "Medium", "Large"], &value));
    assert_eq!(radio.current_index(), Some(0));
}

#[test]
fn unmatched_controlled_value_has_no_selection_until_user_moves() {
    let value = State::new("Unknown".to_string());
    let mut radio = Radio::group("size", ["Small", "Medium"], &value);
    assert_eq!(radio.current_value(), None);

    let _ = radio.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(radio.current_value().as_deref(), Some("Small"));
    assert_eq!(value.get(), "Small");
}

#[test]
fn uncontrolled_radio_keeps_runtime_selection_across_reconcile() {
    let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(1);
    let _ = radio.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(radio.current_value().as_deref(), Some("C"));

    radio.sync_from(Radio::new().options(["A", "B", "C", "D"]));
    assert_eq!(radio.current_value().as_deref(), Some("C"));
}

#[test]
fn external_radio_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new("A".to_string());
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Radio::group("choice", ["A", "B"], &value))
    }));
    let root = tree.root_id().expect("radio root");
    tree.reset_invalidation();

    value.set("B".to_string());
    assert!(tree.take_reconcile_requested());
    let next =
        ViewAdapter::capture_root(|| ViewNode::leaf(Radio::group("choice", ["A", "B"], &value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let radio = tree
        .get(root)
        .expect("radio node")
        .component()
        .as_any()
        .downcast_ref::<Radio>()
        .expect("Radio component");
    assert_eq!(radio.current_value().as_deref(), Some("B"));
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_drive_radio_layout_and_hit_rows() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut radio = with_config(&large, || {
        Radio::new()
            .options(["A", "B"])
            .default_selected(1)
            .vertical()
    });

    assert_eq!(radio.measure(max).h, 80.0);
    assert_eq!(
        radio.on_event(&SystemEvent::PointerDown {
            pos: Point::new(4.0, 39.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(radio.current_index(), Some(0));

    let small = with_config(&large, || {
        Radio::new()
            .options(["A", "B"])
            .size(ControlSize::Small)
            .vertical()
    });
    assert_eq!(small.measure(max).h, 48.0);
}

#[test]
fn radio_cjk_option_width_uses_unicode_metrics_for_every_size() {
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    for (size, expected) in [
        (ControlSize::Small, Size::new(135.04999, 24.0)),
        (ControlSize::Medium, Size::new(168.0, 32.0)),
        (ControlSize::Large, Size::new(199.70665, 40.0)),
    ] {
        assert_size_close(
            Radio::new()
                .options(["苹果", "香蕉", "樱桃"])
                .size(size)
                .measure(max),
            expected,
        );
    }
}

#[test]
fn focused_radio_draws_an_outer_ring_around_the_selected_indicator() {
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut radio = Radio::new().options(["A", "B"]).default_selected(0);
    let measured = radio.measure(max);
    let frame = Rect::new(8.0, 8.0, measured.w, measured.h);
    let (unfocused, stride) = render_radio_pixels(&radio, frame);

    assert_eq!(radio.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    let (focused, _) = render_radio_pixels(&radio, frame);

    assert_ne!(
        pixel_at(&unfocused, stride, 15, 16),
        pixel_at(&focused, stride, 15, 16),
        "focus must add a visible ring outside the selected radio indicator"
    );
}

#[test]
fn radio_arrow_navigation_wraps_at_both_group_edges() {
    let mut radio = Radio::new().options(["A", "B", "C"]).default_selected(2);

    assert_eq!(
        radio.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(radio.current_index(), Some(0));

    assert_eq!(
        radio.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(radio.current_index(), Some(2));
}
