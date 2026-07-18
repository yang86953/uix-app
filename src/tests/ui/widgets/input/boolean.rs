use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::painting::theme::IColorTokens;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{Checkbox, Switch};
use crate::ui::{with_config, ComponentConfig};

fn assert_size_close(actual: Size, expected: Size) {
    assert!(
        (actual.w - expected.w).abs() < 0.001 && (actual.h - expected.h).abs() < 0.001,
        "expected {expected:?}, got {actual:?}"
    );
}

fn render_switch_pixels(switch: &Switch, frame: Rect) -> (Vec<u32>, usize) {
    const SURFACE_WIDTH: i32 = 96;
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
    WidgetRender::render(switch, frame, &mut ctx, &tree);
    drop(ctx);
    (canvas.surface().pixels().to_vec(), SURFACE_WIDTH as usize)
}

fn pixel_at(pixels: &[u32], stride: usize, x: usize, y: usize) -> u32 {
    pixels[y * stride + x]
}

#[test]
fn checkbox_binding_writes_user_changes_and_reads_external_updates() {
    let checked = State::new(false);
    let mut checkbox = Checkbox::new("Agree").checked(&checked);

    assert_eq!(
        checkbox.on_event(&SystemEvent::PointerDown {
            pos: Point::new(1.0, 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(checkbox.is_checked());
    assert!(checked.get());

    checked.set(false);
    checkbox.sync_from(Checkbox::new("Agree").checked(&checked));
    assert!(!checkbox.is_checked());
}

#[test]
fn switch_binding_writes_keyboard_changes_and_preserves_uncontrolled_runtime_value() {
    let checked = State::new(false);
    let mut switch = Switch::new().checked(&checked);

    assert_eq!(
        switch.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(switch.is_checked());
    assert!(checked.get());

    let mut uncontrolled = Switch::new().default_checked(true);
    let _ = uncontrolled.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Space,
        mods: KeyMod::NONE,
    });
    uncontrolled.sync_from(Switch::new().default_checked(true));
    assert!(!uncontrolled.is_checked());
}

#[test]
fn external_boolean_state_reconciles_and_invalidates_the_bound_node() {
    let checked = State::new(false);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Checkbox::new("Agree").checked(&checked))
    }));
    let root = tree.root_id().expect("checkbox root");
    tree.reset_invalidation();

    checked.set(true);
    assert!(tree.take_reconcile_requested());
    let next =
        ViewAdapter::capture_root(|| ViewNode::leaf(Checkbox::new("Agree").checked(&checked)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let checkbox = tree
        .get(root)
        .expect("checkbox node")
        .component()
        .as_any()
        .downcast_ref::<Checkbox>()
        .expect("Checkbox component");
    assert!(checkbox.is_checked());
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_reaches_boolean_controls_and_explicit_size_wins() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let (checkbox, switch) = with_config(&large, || (Checkbox::new("Agree"), Switch::new()));

    assert_eq!(checkbox.measure(max).h, 40.0);
    assert_eq!(switch.measure(max).h, 40.0);

    let (checkbox, switch) = with_config(&large, || {
        (
            Checkbox::new("Agree").size(ControlSize::Small),
            Switch::new().size(ControlSize::Small),
        )
    });
    assert_eq!(checkbox.measure(max).h, 24.0);
    assert_eq!(switch.measure(max).h, 24.0);
}

#[test]
fn checkbox_cjk_width_and_empty_label_gap_follow_visible_content() {
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));

    assert_size_close(
        Checkbox::new("同意协议").measure(max),
        Size::new(74.0, 32.0),
    );
    assert_size_close(
        Checkbox::new("同意协议")
            .size(ControlSize::Small)
            .measure(max),
        Size::new(67.25, 24.0),
    );
    assert_size_close(
        Checkbox::new("同意协议")
            .size(ControlSize::Large)
            .measure(max),
        Size::new(80.75, 40.0),
    );
    assert_size_close(Checkbox::new("").measure(max), Size::new(16.0, 32.0));
}

#[test]
fn switch_thumb_keeps_two_pixel_inset_for_every_size_and_state() {
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    for (size, track_height) in [
        (ControlSize::Small, 16usize),
        (ControlSize::Medium, 22usize),
        (ControlSize::Large, 28usize),
    ] {
        for checked in [false, true] {
            let switch = Switch::new().size(size).default_checked(checked);
            let measured = switch.measure(max);
            let frame = Rect::new(8.0, 8.0, measured.w, measured.h);
            let (pixels, stride) = render_switch_pixels(&switch, frame);
            let track_top = 8 + (measured.h as usize - track_height) / 2;
            let track_left = 8usize;
            let track_width = measured.w as usize;
            let thumb_diameter = track_height - 4;
            let thumb_left = if checked {
                track_left + track_width - 2 - thumb_diameter
            } else {
                track_left + 2
            };
            let thumb_right = thumb_left + thumb_diameter - 1;
            let thumb_center_x = thumb_left + thumb_diameter / 2;
            let thumb_center_y = track_top + track_height / 2;
            let thumb_top = track_top + 2;
            let thumb_bottom = track_top + track_height - 3;
            let thumb_color = DesignTokens::antd_light()
                .color_bg_elevated()
                .premultiplied();
            let top_edge = pixel_at(&pixels, stride, thumb_center_x, thumb_top);
            let left_edge = pixel_at(&pixels, stride, thumb_left, thumb_center_y);

            assert_eq!(
                pixel_at(&pixels, stride, thumb_center_x, thumb_top - 1),
                pixel_at(&pixels, stride, thumb_center_x, thumb_bottom + 1),
                "{size:?} checked={checked} must leave equal track pixels above and below the thumb"
            );
            assert_ne!(
                top_edge,
                pixel_at(&pixels, stride, thumb_center_x, thumb_top - 1),
                "{size:?} checked={checked} thumb edge must begin after the two-pixel inset"
            );
            assert_eq!(
                pixel_at(&pixels, stride, thumb_center_x, thumb_bottom),
                top_edge,
                "{size:?} checked={checked} thumb must have symmetric top and bottom edge coverage"
            );
            assert_eq!(
                pixel_at(&pixels, stride, thumb_left - 1, thumb_center_y),
                pixel_at(&pixels, stride, thumb_right + 1, thumb_center_y),
                "{size:?} checked={checked} must leave equal track pixels beside the thumb"
            );
            assert_eq!(
                pixel_at(&pixels, stride, thumb_right, thumb_center_y),
                left_edge,
                "{size:?} checked={checked} thumb must have symmetric left and right edge coverage"
            );
            assert_eq!(
                pixel_at(
                    &pixels,
                    stride,
                    thumb_center_x,
                    (thumb_top + thumb_bottom) / 2,
                ),
                thumb_color,
                "{size:?} checked={checked} thumb center must remain opaque"
            );
        }
    }
}
