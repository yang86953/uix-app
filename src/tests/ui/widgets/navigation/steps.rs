use crate::tests::common::*;
use crate::ui::widgets::navigation::steps::*;

fn key_event(key: KeyCode) -> SystemEvent {
    SystemEvent::KeyDown {
        key,
        mods: KeyMod::NONE,
    }
}

#[test]
fn steps_are_focusable_and_keyboard_navigation_follows_direction() {
    let mut steps = Steps::new(vec![Step::new("Start"), Step::new("Done")]);
    let event = key_event(KeyCode::Right);

    assert_eq!(WidgetComponent::tab_index(&steps), 1);
    assert_eq!(steps.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(steps.on_event(&event), EventResult::Handled);
    assert_eq!(steps.get_current(), 1);
    assert_eq!(
        steps
            .semantic_event(ComponentId::new(2), &event)
            .and_then(|event| { event.text_payload().map(str::to_owned) }),
        Some("1".to_string())
    );
    assert_eq!(
        steps.on_event(&key_event(KeyCode::Down)),
        EventResult::NotHandled
    );
}

#[test]
fn vertical_steps_render_contract_and_pointer_selection_are_reachable() {
    let mut steps = Steps::new(vec![
        Step::new("Start"),
        Step::new("Middle").description("Working"),
        Step::new("Done"),
    ])
    .vertical();

    assert_eq!(
        steps.measure(Constraints::loose(Size::new(500.0, 500.0))),
        Size::new(200.0, 240.0)
    );
    assert_eq!(
        steps.on_event(&SystemEvent::PointerDown {
            pos: Point::new(30.0, 100.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(steps.get_current(), 1);
    assert_eq!(
        steps.on_event(&key_event(KeyCode::Right)),
        EventResult::NotHandled
    );
    assert_eq!(
        steps.on_event(&key_event(KeyCode::Down)),
        EventResult::Handled
    );
    assert_eq!(steps.get_current(), 2);
}

#[test]
fn steps_snapshot_and_accessibility_expose_current_step() {
    let steps = Steps::new(vec![Step::new("Start"), Step::new("Review")]).current(1);
    let fields = steps.snapshot_fields();

    assert!(matches!(
        fields,
        SnapshotFields::Steps {
            current: 1,
            direction: StepsDirection::Horizontal,
            ..
        }
    ));
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, crate::ui::AccessibilityRole::Navigation);
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Review"));
    assert_eq!(accessibility.state.value_now, Some(2.0));
}

#[test]
fn empty_steps_are_not_focusable_and_current_is_clamped() {
    let empty = Steps::new(Vec::new()).current(99);
    assert_eq!(WidgetComponent::tab_index(&empty), 0);
    assert_eq!(empty.get_current(), 0);

    let steps = Steps::new(vec![Step::new("Only")]).current(99);
    assert_eq!(steps.get_current(), 0);
}
