use crate::tests::common::*;
use crate::ui::widgets::ThemeToggle;
use crate::ui::AccessibilityRole;

#[test]
fn theme_toggle_keyboard_activation_emits_theme_name() {
    let mut toggle = ThemeToggle::new();
    let enter = SystemEvent::KeyDown {
        key: KeyCode::Enter,
        mods: KeyMod::NONE,
    };

    assert_eq!(WidgetComponent::tab_index(&toggle), 1);
    assert_eq!(toggle.on_event(&SystemEvent::FocusIn), EventResult::Handled);
    assert_eq!(toggle.on_event(&enter), EventResult::Handled);
    assert!(toggle.is_dark());
    assert_eq!(
        toggle
            .semantic_event(ComponentId::new(3), &enter)
            .and_then(|event| event.text_payload().map(str::to_owned)),
        Some("dark".to_string())
    );

    assert_eq!(
        toggle.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Space,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert!(!toggle.is_dark());
}

#[test]
fn theme_toggle_snapshot_and_accessibility_follow_runtime_state() {
    let mut toggle = ThemeToggle::new().dark(false);
    toggle.on_event(&SystemEvent::PointerDown {
        pos: Point::new(4.0, 4.0),
        button: MouseButton::Left,
        mods: KeyMod::NONE,
    });
    let fields = toggle.snapshot_fields();

    assert_eq!(fields, SnapshotFields::ThemeToggle { dark: true });
    let accessibility = fields.accessibility();
    assert_eq!(accessibility.role, AccessibilityRole::Button);
    assert_eq!(accessibility.state.checked, Some(true));
    assert_eq!(accessibility.state.value_text.as_deref(), Some("dark"));
}

#[test]
fn theme_toggle_ignores_unrelated_keys() {
    let mut toggle = ThemeToggle::new();

    assert_eq!(
        toggle.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert!(!toggle.is_dark());
}
