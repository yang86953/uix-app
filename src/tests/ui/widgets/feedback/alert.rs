use crate::tests::common::*;
use crate::ui::widgets::Alert;

#[test]
fn closable_alert_ignores_body_click_and_dismisses_from_close_region() {
    let mut alert = Alert::new("network unavailable").closable();
    let body = SystemEvent::PointerDown {
        pos: Point::new(20.0, 18.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };
    let close = SystemEvent::PointerDown {
        pos: Point::new(280.0, 18.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    };

    assert_eq!(alert.on_event(&body), EventResult::NotHandled);
    assert!(alert.is_visible());
    assert_eq!(alert.on_event(&close), EventResult::Handled);
    assert!(!alert.is_visible());
    assert!(alert.take_layout_request());
    assert!(!alert.take_layout_request());
    assert_eq!(
        alert
            .semantic_event(ComponentId::new(5), &close)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("closed".into())
    );
}

#[test]
fn closable_alert_is_keyboard_accessible_and_can_be_reopened() {
    let mut alert = Alert::new("saved").closable();
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&alert), 1);
    assert_eq!(alert.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(alert.on_event(&enter), EventResult::Handled);
    assert!(!alert.is_visible());
    assert_eq!(WidgetComponent::tab_index(&alert), 0);

    alert.open();
    assert!(alert.is_visible());
    assert!(alert.take_layout_request());
    assert_eq!(WidgetComponent::tab_index(&alert), 1);
}

#[test]
fn non_closable_alert_does_not_claim_close_interaction() {
    let mut alert = Alert::new("informational");

    assert_eq!(WidgetComponent::tab_index(&alert), 0);
    assert_eq!(
        alert.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Escape,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert!(alert.is_visible());
}
