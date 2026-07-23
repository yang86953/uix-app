use crate::draw::backend::cpu::pixel_surface::PixelSurface;
use crate::draw::backend::cpu::shared_rasterizer::SharedRasterizer;
use crate::draw::geometry::spatial::Orientation;
use crate::tests::common::*;
use crate::ui::state::State;
use crate::ui::view::{ViewAdapter, ViewNode};
use crate::ui::widgets::InputNumber;

fn render_input_number(
    input: &InputNumber,
    frame: Rect,
    surface_size: (i32, i32),
    measure_text: &str,
) -> (Vec<u32>, f32) {
    let mut canvas = SharedRasterizer::new(PixelSurface::new(surface_size.0, surface_size.1));
    let mut fonts = FontService::new();
    let font = fonts
        .load_font(include_bytes!("../../../../../assets/fonts/lucide.ttf"))
        .expect("load deterministic test font");
    let images = ImageService::new();
    let tokens = DesignTokens::antd_light();
    let tree = WidgetTree::new();

    let measured;
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
        measured = ctx.measure_text(measure_text, 14.0).w;
        WidgetRender::render(input, frame, &mut ctx, &tree);
    }
    (canvas.surface().pixels().to_vec(), measured)
}

#[test]
fn bound_integer_value_writes_keyboard_changes_and_reads_external_updates() {
    let value = State::new(4i32);
    let mut input = InputNumber::new()
        .placeholder("Count")
        .min(0.0)
        .max(10.0)
        .step(2.0)
        .value(&value);

    assert_eq!(input.current_value(), 4.0);
    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), 6.0);
    assert_eq!(value.get(), 6);

    value.set(20);
    input.sync_from(
        InputNumber::new()
            .placeholder("Count")
            .min(0.0)
            .max(10.0)
            .step(2.0)
            .value(&value),
    );
    assert_eq!(input.current_value(), 10.0);

    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Down,
        mods: KeyMod::NONE,
    });
    assert_eq!(input.current_value(), 8.0);
    assert_eq!(value.get(), 8);
}

#[test]
fn integer_binding_normalizes_fractional_steps_before_publishing() {
    let value = State::new(1i32);
    let mut input = InputNumber::new().step(0.6).value(&value);

    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 2.0);
    assert_eq!(value.get(), 2);
}

#[test]
fn uncontrolled_value_survives_reconcile() {
    let mut input = InputNumber::new()
        .placeholder("Count")
        .min(0.0)
        .max(100.0)
        .step(1.0);
    let _ = input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(1.0, 1.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });

    input.sync_from(
        InputNumber::new()
            .placeholder("Count")
            .min(0.0)
            .max(100.0)
            .step(1.0),
    );

    assert_eq!(input.current_value(), 1.0);
}

#[test]
fn external_state_reconciles_and_invalidates_the_bound_node() {
    let value = State::new(4.0);
    let mut tree = ViewAdapter::build_nodes(ViewAdapter::capture_root(|| {
        ViewNode::leaf(InputNumber::new().min(0.0).max(10.0).value(&value))
    }));
    let root = tree.root_id().expect("input number root");
    tree.reset_invalidation();

    value.set(7.0);
    assert!(tree.take_reconcile_requested());
    let next = ViewAdapter::capture_root(|| {
        ViewNode::leaf(InputNumber::new().min(0.0).max(10.0).value(&value))
    });
    ViewAdapter::reconcile_nodes(&mut tree, next);

    let input = tree
        .get(root)
        .expect("input number node")
        .component()
        .as_any()
        .downcast_ref::<InputNumber>()
        .expect("InputNumber component");
    assert_eq!(input.current_value(), 7.0);
    assert!(tree
        .invalidation()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .node_needs_paint(root));
}

#[test]
fn input_number_accepts_platform_text_and_enter_commits_the_buffer() {
    let value = State::new(4.0f64);
    let mut input = InputNumber::new().min(0.0).max(10.0).value(&value);
    assert!(input
        .as_text_input()
        .expect("InputNumber text input capability")
        .accepts_text_input());

    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "7.5".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 7.5);
    assert_eq!(value.get(), 7.5);
}

#[test]
fn formatter_changes_display_and_accessibility_without_polluting_numeric_editing() {
    let mut input = InputNumber::new()
        .default_value(12.5f64)
        .formatter(|value| format!("{value:.1} %"));
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::InputNumber {
            formatted: true,
            display_value: Some(display_value),
            ..
        } if display_value == "12.5 %"
    ));
    let accessibility = input.snapshot_fields().accessibility();
    assert_eq!(accessibility.state.value_now, Some(12.5));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("12.5 %"));

    let _ = input.on_event(&SystemEvent::FocusIn);
    let (_, measured) =
        render_input_number(&input, Rect::new(0.0, 0.0, 160.0, 32.0), (164, 36), "12.5");
    let caret = input
        .as_text_input()
        .expect("InputNumber text input capability")
        .text_input_cursor_rect();
    assert!(
        (caret.x - (12.0 + measured)).abs() < 0.01,
        "editing must expose the raw parseable number instead of the formatted projection"
    );

    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "75".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(input.current_value(), 12.75);
    assert_eq!(
        input
            .semantic_event(ComponentId::new(21), &SystemEvent::FocusIn)
            .expect("formatted input commit must retain numeric Change semantics")
            .text_payload(),
        Some("12.75")
    );
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::InputNumber {
            display_value: Some(display_value),
            ..
        } if display_value == "12.8 %"
    ));
}

#[test]
fn formatter_reconcile_preserves_the_uncommitted_buffer_and_pointer_only_steps() {
    let mut input = InputNumber::new()
        .default_value(4.0f64)
        .formatter(|value| format!("${value}"));
    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "7".to_owned(),
    });

    input.sync_from(InputNumber::new().formatter(|value| format!("{value} kg")));
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });
    assert_eq!(input.current_value(), 7.0);
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::InputNumber {
            display_value: Some(display_value),
            ..
        } if display_value == "7 kg"
    ));

    input.sync_from(
        InputNumber::new()
            .keyboard(false)
            .formatter(|value| format!("{value} kg")),
    );
    assert_eq!(
        input.on_event(&SystemEvent::PointerDown {
            pos: Point::new(100.0, 5.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::InputNumber {
            keyboard: false,
            display_value: Some(display_value),
            ..
        } if display_value == "8 kg"
    ));
}

#[test]
fn input_number_focus_out_commits_and_invalid_text_preserves_value() {
    let value = State::new(2.0f64);
    let mut input = InputNumber::new().value(&value);
    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "8".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::FocusOut);
    assert_eq!(value.get(), 8.0);

    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "..".to_owned(),
    });
    let _ = input.on_event(&SystemEvent::FocusOut);
    assert_eq!(value.get(), 8.0);
}

#[test]
fn input_number_pointer_step_buttons_use_the_configured_step() {
    let value = State::new(4i32);
    let mut input = InputNumber::new().step(2.0).value(&value);
    let button_x = 100.0;

    assert_eq!(
        input.on_event(&SystemEvent::PointerDown {
            pos: Point::new(button_x, 5.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(value.get(), 6);

    let _ = input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(button_x, 25.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    assert_eq!(value.get(), 4);
}

#[test]
fn keyboard_false_keeps_only_pointer_and_semantic_step_inputs() {
    let mut input = InputNumber::new()
        .default_value(4.0f64)
        .step(2.0)
        .keyboard(false);

    assert!(!input
        .as_text_input()
        .expect("InputNumber text input capability")
        .accepts_text_input());
    assert_eq!(input.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Backspace,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        input.on_event(&SystemEvent::TextInput {
            text: "7".to_owned(),
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        input.on_event(&SystemEvent::Paste {
            text: "8".to_owned(),
        }),
        EventResult::NotHandled
    );
    assert_eq!(input.current_value(), 4.0);

    let _ = render_input_number(&input, Rect::new(0.0, 0.0, 112.0, 32.0), (120, 40), "4");
    assert_eq!(
        input
            .as_text_input()
            .expect("InputNumber text input capability")
            .text_input_cursor_rect(),
        Rect::zero()
    );
    assert!(matches!(
        input.snapshot_fields(),
        SnapshotFields::InputNumber {
            keyboard: false,
            ..
        }
    ));

    assert_eq!(
        input.on_event(&SystemEvent::PointerDown {
            pos: Point::new(100.0, 5.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), 6.0);
}

#[test]
fn disabling_keyboard_during_reconcile_discards_the_uncommitted_buffer() {
    let mut input = InputNumber::new().default_value(4.0f64);
    let _ = input.on_event(&SystemEvent::FocusIn);
    let _ = input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    let _ = input.on_event(&SystemEvent::TextInput {
        text: "7".to_owned(),
    });

    input.sync_from(InputNumber::new().keyboard(false));
    assert_eq!(
        input.on_event(&SystemEvent::PointerDown {
            pos: Point::new(100.0, 5.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(input.current_value(), 5.0);
}

#[test]
fn disabled_input_number_does_not_request_text_input_or_handle_steps() {
    let mut input = InputNumber::new().disabled(true);
    assert!(!input
        .as_text_input()
        .expect("InputNumber text input capability")
        .accepts_text_input());
    assert_eq!(
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
}

#[test]
fn input_number_preserves_more_than_two_fraction_digits_across_focus_commit() {
    let mut input = InputNumber::new().default_value(1.2345f64);

    input.on_event(&SystemEvent::FocusIn);
    input.on_event(&SystemEvent::FocusOut);

    assert_eq!(input.current_value(), 1.2345);
}

#[test]
fn repeated_decimal_steps_do_not_accumulate_binary_display_artifacts() {
    let mut input = InputNumber::new().default_value(0.0f64).step(0.1);

    for _ in 0..3 {
        input.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Up,
            mods: KeyMod::NONE,
        });
    }

    assert_eq!(input.current_value(), 0.3);
}

#[test]
fn repeated_pointer_focus_keeps_the_in_progress_numeric_buffer() {
    let mut input = InputNumber::new().default_value(4.0f64);
    input.on_event(&SystemEvent::FocusIn);
    input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    input.on_event(&SystemEvent::TextInput {
        text: "7".to_string(),
    });

    input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(12.0, 16.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 7.0);
}

#[test]
fn keyboard_step_commits_the_edit_buffer_before_incrementing() {
    let mut input = InputNumber::new().default_value(4.0f64);
    input.on_event(&SystemEvent::FocusIn);
    input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    input.on_event(&SystemEvent::TextInput {
        text: "7".to_string(),
    });

    input.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Up,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 8.0);
}

#[test]
fn constrained_large_step_buttons_split_the_rendered_height() {
    let mut input = InputNumber::new()
        .default_value(4.0f64)
        .size(ControlSize::Large);
    render_input_number(&input, Rect::new(240.0, 160.0, 100.0, 24.0), (360, 200), "");

    input.on_event(&SystemEvent::PointerDown {
        pos: Point::new(90.0, 18.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });

    assert_eq!(input.current_value(), 3.0);
}

#[test]
fn focused_input_number_draws_the_outer_border_around_the_step_area() {
    let normal = InputNumber::new().default_value(42.0f64);
    let (normal_pixels, _) =
        render_input_number(&normal, Rect::new(0.0, 0.0, 100.0, 32.0), (104, 36), "");
    let mut focused = InputNumber::new().default_value(42.0f64);
    focused.on_event(&SystemEvent::FocusIn);
    let (focused_pixels, _) =
        render_input_number(&focused, Rect::new(0.0, 0.0, 100.0, 32.0), (104, 36), "");

    let right_edge_center = 16 * 104 + 99;
    assert_ne!(
        normal_pixels[right_edge_center], focused_pixels[right_edge_center],
        "focus border must include the right edge of the step area"
    );
}

#[test]
fn disabled_input_number_uses_a_distinct_control_background() {
    let enabled = InputNumber::new();
    let (enabled_pixels, _) =
        render_input_number(&enabled, Rect::new(0.0, 0.0, 100.0, 32.0), (104, 36), "");
    let disabled = InputNumber::new().disabled(true);
    let (disabled_pixels, _) =
        render_input_number(&disabled, Rect::new(0.0, 0.0, 100.0, 32.0), (104, 36), "");

    let input_background = 16 * 104 + 20;
    assert_ne!(
        enabled_pixels[input_background], disabled_pixels[input_background],
        "disabled input body must not reuse the enabled background"
    );
}

#[test]
fn input_number_caret_uses_the_rendered_font_width() {
    let mut input = InputNumber::new().default_value(111.11f64);
    input.on_event(&SystemEvent::FocusIn);
    let (_, measured) = render_input_number(
        &input,
        Rect::new(0.0, 0.0, 160.0, 32.0),
        (164, 36),
        "111.11",
    );
    let caret = input
        .as_text_input()
        .expect("InputNumber text input capability")
        .text_input_cursor_rect();

    assert!(
        (caret.x - (12.0 + measured)).abs() < 0.01,
        "caret must follow actual glyph advance: caret={caret:?}, measured={measured}"
    );
    assert_eq!(caret.y, 4.0);
    assert_eq!(caret.h, 24.0);
}

#[test]
fn every_input_number_size_preserves_the_text_body_beside_its_square_step_area() {
    for (size, expected) in [
        (ControlSize::Small, Size::new(104.0, 24.0)),
        (ControlSize::Medium, Size::new(112.0, 32.0)),
        (ControlSize::Large, Size::new(120.0, 40.0)),
    ] {
        assert_eq!(
            InputNumber::new()
                .size(size)
                .measure(Constraints::loose(Size::new(200.0, 80.0))),
            expected
        );
    }
}
