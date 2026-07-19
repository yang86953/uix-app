use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::{RangeSlider, RangeSliderThumb, Slider};
use crate::ui::AccessibilityRole;

fn render_range_slider(slider: &RangeSlider, frame: Rect) -> Vec<u32> {
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
fn public_range_slider_binds_both_states_and_keyboard_uses_active_thumb() {
    let start = State::new(20.0);
    let end = State::new(80.0);
    let mut slider = Slider::range(0.0..=100.0)
        .step(10.0)
        .start(&start)
        .end(&end);

    assert_eq!(slider.current_range(), (20.0, 80.0));
    let _ = slider.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_range(), (30.0, 80.0));
    assert_eq!(start.get(), 30.0);

    let _ = render_range_slider(&slider, Rect::new(0.0, 0.0, 200.0, 32.0));
    let _ = slider.on_event(&SystemEvent::PointerDown {
        pos: Point::new(156.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = slider.on_event(&SystemEvent::PointerUp {
        pos: Point::new(156.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.active_thumb(), RangeSliderThumb::End);
    let _ = slider.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_range(), (30.0, 70.0));
    assert_eq!(end.get(), 70.0);
}

#[test]
fn thumbs_do_not_cross_and_captured_drag_survives_pointer_leave() {
    let start = State::new(20.0);
    let end = State::new(80.0);
    let mut slider = Slider::range(0.0..=100.0)
        .step(10.0)
        .start(&start)
        .end(&end);
    let _ = render_range_slider(&slider, Rect::new(0.0, 0.0, 200.0, 32.0));

    let _ = slider.on_event(&SystemEvent::PointerDown {
        pos: Point::new(44.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = slider.on_event(&SystemEvent::PointerLeave);
    let _ = slider.on_event(&SystemEvent::PointerMove {
        pos: Point::new(194.0, 16.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_range(), (80.0, 80.0));
    assert_eq!(start.get(), 80.0);
    let overlapped_pixels = render_range_slider(&slider, Rect::new(0.0, 0.0, 200.0, 32.0));
    assert_eq!(
        overlapped_pixels[16 * 240 + 156],
        DesignTokens::antd_light().color_primary_hover.to_rgba(),
        "the active start thumb must paint above the overlapped end thumb"
    );

    let _ = slider.on_event(&SystemEvent::PointerUp {
        pos: Point::new(194.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = slider.on_event(&SystemEvent::PointerMove {
        pos: Point::new(6.0, 16.0),
        mods: KeyMod::NONE,
    });
    assert_eq!(slider.current_range(), (80.0, 80.0));
}

#[test]
fn reversed_external_values_converge_start_to_end_in_snapshot_and_accessibility() {
    let start = State::new(80.0);
    let end = State::new(20.0);
    let slider = Slider::range(0.0..=100.0).start(&start).end(&end);

    assert_eq!(slider.current_range(), (20.0, 20.0));
    let fields = slider.snapshot_fields();
    assert_eq!(
        fields,
        SnapshotFields::RangeSlider {
            min: 0.0,
            max: 100.0,
            step: 1.0,
            start: 20.0,
            end: 20.0,
            active_thumb: RangeSliderThumb::Start,
            size: ControlSize::Medium,
        }
    );
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Slider);
    assert_eq!(accessibility.state.value_now, Some(20.0));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("20..20"));
}

#[test]
fn range_slider_reconcile_preserves_instance_and_reads_new_bound_values() {
    let start = State::new(10.0);
    let end = State::new(90.0);
    let mut tree = ViewAdapter::build_nodes(ViewNode::leaf(
        Slider::range(0.0..=100.0).start(&start).end(&end),
    ));
    let root = tree.root_id().expect("range slider root");
    let before = tree
        .get(root)
        .expect("range slider node")
        .component()
        .as_any()
        .downcast_ref::<RangeSlider>()
        .expect("RangeSlider") as *const RangeSlider;

    start.set(25.0);
    end.set(75.0);
    ViewAdapter::reconcile_nodes(
        &mut tree,
        ViewNode::leaf(
            Slider::range(-10.0..=90.0)
                .step(5.0)
                .start(&start)
                .end(&end)
                .size(ControlSize::Large),
        ),
    );
    let slider = tree
        .get(root)
        .expect("range slider node")
        .component()
        .as_any()
        .downcast_ref::<RangeSlider>()
        .expect("RangeSlider");
    assert_eq!(slider as *const RangeSlider, before);
    assert_eq!(slider.current_range(), (25.0, 75.0));
    assert!(matches!(
        slider.snapshot_fields(),
        SnapshotFields::RangeSlider {
            min: -10.0,
            max: 90.0,
            step: 5.0,
            size: ControlSize::Large,
            ..
        }
    ));
}

#[test]
fn constrained_range_slider_paint_and_hit_stay_inside_actual_frame() {
    let start = State::new(25.0);
    let end = State::new(75.0);
    let mut slider = Slider::range(0.0..=100.0).start(&start).end(&end);
    let frame = Rect::new(0.0, 0.0, 8.0, 10.0);
    let pixels = render_range_slider(&slider, frame);

    for y in 0..64 {
        for x in 0..240 {
            if x >= frame.w as usize || y >= frame.h as usize {
                assert_eq!(pixels[y * 240 + x], 0, "paint leaked at ({x}, {y})");
            }
        }
    }
    assert_eq!(
        slider.on_event(&SystemEvent::PointerDown {
            pos: Point::new(4.0, 12.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(slider.current_range(), (25.0, 75.0));
}
