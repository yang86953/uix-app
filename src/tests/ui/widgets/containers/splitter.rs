use crate::tests::common::*;
use crate::ui::widgets::containers::splitter::Splitter;
use crate::ui::AccessibilityRole;

fn capture_layout(splitter: &Splitter, frame: Rect) {
    let _ = splitter.layout_children(frame, &[], &WidgetTree::new());
}

#[test]
fn splitter_clamps_empty_panel_declaration_and_minimum_sizes() {
    let splitter = Splitter::new().panels(0).min_size(0, f32::NAN);

    assert_eq!(splitter.ratios(), &[1.0]);
    assert_eq!(WidgetComponent::tab_index(&splitter), 0);
    assert_eq!(
        splitter.snapshot_fields(),
        SnapshotFields::Splitter {
            vertical: false,
            panel_count: 1,
            min_sizes: vec![0.0],
            handle_size: 6.0,
            ratios: vec![1.0],
            active_handle: 0,
        }
    );
}

#[test]
fn splitter_keyboard_moves_active_handle_and_emits_ratios() {
    let mut splitter = Splitter::new().min_size(0, 50.0).min_size(1, 50.0);
    capture_layout(&splitter, Rect::new(20.0, 30.0, 306.0, 200.0));
    let right = SystemEvent::KeyDown {
        key: KeyCode::Right,
        mods: KeyMod::NONE,
    };

    assert_eq!(
        splitter.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(splitter.on_event(&right), EventResult::Handled);
    assert!(splitter.ratios()[0] > 0.5);
    assert!(splitter.take_layout_request());
    assert!(!splitter.take_layout_request());
    assert_eq!(
        splitter
            .semantic_event(ComponentId::new(7), &right)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("0.526667,0.473333".to_string())
    );
}

#[test]
fn splitter_keyboard_obeys_orientation_and_adjacent_minimums() {
    let mut splitter = Splitter::new()
        .vertical(true)
        .min_size(0, 80.0)
        .min_size(1, 60.0);
    capture_layout(&splitter, Rect::new(40.0, 60.0, 200.0, 206.0));

    assert_eq!(
        splitter.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Right,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        splitter.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Home,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!((splitter.ratios()[0] - 0.4).abs() < 0.0001);
    assert_eq!(splitter.ratios()[1], 0.6);
}

#[test]
fn splitter_keyboard_can_select_each_handle_and_exposes_slider_value() {
    let mut splitter = Splitter::new().panels(3);
    capture_layout(&splitter, Rect::new(0.0, 0.0, 312.0, 200.0));

    assert_eq!(
        splitter.on_event(&SystemEvent::KeyDown {
            key: KeyCode::PageDown,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(splitter.active_handle(), 1);
    let accessibility = splitter.snapshot_fields().accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Slider);
    assert!((accessibility.state.value_now.expect("boundary") - 2.0 / 3.0).abs() < 0.0001);
    assert_eq!(
        accessibility.state.value_text.as_deref(),
        Some("0.333333,0.333333,0.333333")
    );
}

#[test]
fn splitter_pointer_hit_uses_the_global_frame_origin() {
    let mut splitter = Splitter::new();
    capture_layout(&splitter, Rect::new(100.0, 50.0, 306.0, 180.0));
    let down = SystemEvent::PointerDown {
        pos: Point::new(251.0, 80.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(splitter.on_event(&down), EventResult::Handled);
    assert_eq!(splitter.active_handle(), 0);
    assert!(EventHandler::wants_continuous_pointer_move(&splitter));
    assert_eq!(
        splitter.on_event(&SystemEvent::PointerUp {
            pos: Point::new(251.0, 80.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!EventHandler::wants_continuous_pointer_move(&splitter));
}
