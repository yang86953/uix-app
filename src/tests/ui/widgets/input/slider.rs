use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Slider;
use crate::ui::{with_config, ComponentConfig};

fn render_slider(slider: &Slider, frame: Rect) {
    let mut canvas = CpuCanvas2D::new(PixelSurface::new(240, 64));
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
        240,
        64,
    );
    WidgetRender::render(slider, frame, &mut ctx, &tree);
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
