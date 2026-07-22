use crate::draw::engine::cpu::canvas_2d::CpuCanvas2D;
use crate::draw::engine::cpu::pixel_surface::PixelSurface;
use crate::draw::font::text_backend::estimate_text_metrics;
use crate::draw::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::Rate;
use crate::ui::{with_config, ComponentConfig};

const SURFACE_WIDTH: i32 = 180;
const SURFACE_HEIGHT: i32 = 64;

fn render_rate(rate: &Rate, frame: Rect) -> (Vec<u32>, usize) {
    let mut canvas = CpuCanvas2D::new(PixelSurface::new(SURFACE_WIDTH, SURFACE_HEIGHT));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("bundled Lucide font");
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
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
        );
        WidgetRender::render(rate, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), SURFACE_WIDTH as usize)
}

fn pixel_at(pixels: &[u32], stride: usize, x: usize, y: usize) -> u32 {
    pixels[y * stride + x]
}

#[test]
fn bound_rate_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new(3u32);
    let mut rate = Rate::new().count(5).value(&value);

    assert_eq!(rate.current_value(), 3);
    assert_eq!(
        rate.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(rate.current_value(), 4);
    assert_eq!(value.get(), 4);

    value.set(10);
    rate.sync_from(Rate::new().count(5).value(&value));
    assert_eq!(rate.current_value(), 5);

    let _ = rate.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(rate.current_value(), 4);
    assert_eq!(value.get(), 4);
}

#[test]
fn half_rate_uses_half_step_units_for_state_values() {
    let value = State::new(7u32);
    let mut rate = Rate::new().value(&value).count(4).allow_half();

    assert_eq!(rate.current_value(), 7);

    let _ = rate.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });

    assert_eq!(rate.current_value(), 8);
    assert_eq!(value.get(), 8);
}

#[test]
fn half_rate_accessibility_range_uses_half_step_units() {
    let accessibility = Rate::new()
        .count(4)
        .allow_half()
        .default_value(7)
        .snapshot_fields()
        .accessibility();

    assert_eq!(accessibility.state.value_now, Some(7.0));
    assert_eq!(accessibility.state.value_min, Some(0.0));
    assert_eq!(accessibility.state.value_max, Some(8.0));
}

#[test]
fn uncontrolled_rate_keeps_runtime_value_across_reconcile() {
    let mut rate = Rate::new().count(5).default_value(2);
    let _ = rate.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    });
    assert_eq!(rate.current_value(), 3);

    rate.sync_from(Rate::new().count(4).default_value(1));
    assert_eq!(rate.current_value(), 3);
}

#[test]
fn external_rate_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new(2u32);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(Rate::new().count(5).value(&value))
    }));
    let root = tree.root_id().expect("rate root");
    tree.reset_invalidation();

    value.set(4);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| ViewNode::leaf(Rate::new().count(5).value(&value)));
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let rate = tree
        .get(root)
        .expect("rate node")
        .component()
        .as_any()
        .downcast_ref::<Rate>()
        .expect("Rate component");
    assert_eq!(rate.current_value(), 4);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn provider_size_and_explicit_override_drive_rate_layout_and_half_hit() {
    let large = ComponentConfig::new().component_size(ControlSize::Large);
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let mut rate = with_config(&large, || Rate::new().count(2).allow_half());
    assert_eq!(rate.measure(max).h, 40.0);
    let _ = render_rate(&rate, Rect::new(0.0, 0.0, 57.6, 40.0));

    assert_eq!(
        rate.on_event(&SystemEvent::PointerDown {
            pos: Point::new(14.0, 20.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(rate.current_value(), 1);

    let small = with_config(&large, || Rate::new().size(ControlSize::Small));
    assert_eq!(small.measure(max).h, 24.0);
}

#[test]
fn rate_is_keyboard_focusable_and_tracks_focus_events() {
    let mut rate = Rate::new();
    assert_eq!(WidgetComponent::tab_index(&rate), 1);
    assert_eq!(rate.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        rate.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(rate.current_value(), 1);
    assert_eq!(rate.on_event(&SystemEvent::FocusOut), EventResult::Handled);
}

#[test]
fn half_rate_paints_active_left_half_and_empty_right_half() {
    let frame = Rect::new(8.0, 8.0, 24.0, 32.0);
    let (empty, stride) = render_rate(&Rate::new().count(1).allow_half(), frame);
    let (half, _) = render_rate(&Rate::new().count(1).allow_half().default_value(1), frame);
    let (full, _) = render_rate(&Rate::new().count(1).allow_half().default_value(2), frame);

    let split_x = (frame.x + frame.w * 0.5) as usize;
    let mut left_samples = 0;
    let mut right_samples = 0;
    let mut changed_min_x = usize::MAX;
    let mut changed_max_x = 0;
    for y in frame.y as usize..(frame.y + frame.h) as usize {
        for x in frame.x as usize..(frame.x + frame.w) as usize {
            let empty_pixel = pixel_at(&empty, stride, x, y);
            let full_pixel = pixel_at(&full, stride, x, y);
            if empty_pixel == full_pixel {
                continue;
            }
            changed_min_x = changed_min_x.min(x);
            changed_max_x = changed_max_x.max(x);
            if x < split_x {
                left_samples += 1;
                assert_eq!(
                    pixel_at(&half, stride, x, y),
                    full_pixel,
                    "half-selected glyph must use the active color on its left half"
                );
            } else {
                right_samples += 1;
                assert_eq!(
                    pixel_at(&half, stride, x, y),
                    empty_pixel,
                    "half-selected glyph must preserve the empty color on its right half"
                );
            }
        }
    }
    assert!(
        left_samples > 0 && right_samples > 0,
        "active/empty glyph differences must cross the half-cell split: split={split_x}, changed={changed_min_x}..={changed_max_x}"
    );
}

#[test]
fn default_rate_star_is_owned_by_the_icon_component_path() {
    let source = include_str!("../../../../ui/widgets/input/rate.rs");

    assert!(
        !source.contains('★'),
        "default Rate must not embed a Unicode star"
    );
    assert!(source.contains("\"star\""));
    assert!(source.contains("Icon::paint_in_frame"));
    assert!(!source.contains("icon_char("));
}

#[test]
fn custom_rate_character_reserves_its_unicode_text_width() {
    let max = Constraints::loose(Size::new(1_000.0, 1_000.0));
    let text_width = estimate_text_metrics("推荐", f32::INFINITY, 18.0).max_line_width;
    let measured = Rate::new().count(2).character("推荐").measure(max);

    assert!(
        measured.w >= (text_width + 4.0) * 2.0,
        "each custom character cell must reserve its visible Unicode width: {measured:?}"
    );
    assert_eq!(measured.h, 32.0);
}

#[test]
fn constrained_rate_paint_stays_inside_the_actual_frame() {
    let mut rate = Rate::new()
        .count(5)
        .size(ControlSize::Large)
        .default_value(3);
    let _ = rate.on_event(&SystemEvent::FocusIn);
    let frame = Rect::new(0.0, 0.0, 40.0, 10.0);
    let (pixels, stride) = render_rate(&rate, frame);

    for y in 0..SURFACE_HEIGHT as usize {
        for x in 0..SURFACE_WIDTH as usize {
            if x >= frame.w as usize || y >= frame.h as usize {
                assert_eq!(
                    pixel_at(&pixels, stride, x, y),
                    0,
                    "rate paint leaked at ({x}, {y})"
                );
            }
        }
    }
}

#[test]
fn pointer_hit_uses_the_last_rendered_height_and_rejects_outside_points() {
    let mut rate = Rate::new().count(3).size(ControlSize::Large);
    let frame = Rect::new(0.0, 0.0, 40.0, 10.0);
    let _ = render_rate(&rate, frame);

    assert_eq!(
        rate.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 11.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(rate.current_value(), 0);

    assert_eq!(
        rate.on_event(&SystemEvent::PointerDown {
            pos: Point::new(10.0, 5.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(
        rate.current_value(),
        2,
        "constrained height must scale visual cells and pointer hit geometry together"
    );
}
