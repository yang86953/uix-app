use crate::tests::common::*;
use crate::ui::widgets::Mentions;

fn mentions() -> Mentions {
    Mentions::new("Mention a person").options(vec!["Ada", "Alan", "Grace"])
}

#[test]
fn mentions_keeps_typed_query_in_value_and_filters_suggestions() {
    let mut mentions = mentions();
    assert!(mentions
        .as_text_input()
        .expect("Mentions text input capability")
        .accepts_text_input());

    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "Hello @AL".to_owned(),
    });
    assert_eq!(mentions.value(), "Hello @AL");
    assert!(mentions.is_suggesting());
    assert_eq!(mentions.filtered_options(), &["Alan"]);
    assert!(matches!(
        mentions.snapshot_fields(),
        SnapshotFields::Mentions {
            value,
            suggesting: true,
            ..
        } if value == "Hello @AL"
    ));
    let accessibility = mentions.snapshot_fields().accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("Hello @AL"));
    assert_eq!(accessibility.state.expanded, Some(true));
}

#[test]
fn mentions_backspace_refilters_and_enter_replaces_active_query() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@Adx".to_owned(),
    });
    let _ = mentions.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });
    assert_eq!(mentions.value(), "@Ad");
    assert_eq!(mentions.filtered_options(), &["Ada"]);

    assert_eq!(
        mentions.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(mentions.value(), "@Ada ");
    assert!(!mentions.is_suggesting());
}

#[test]
fn mentions_popup_click_commits_visible_suggestion() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@a".to_owned(),
    });
    assert_eq!(
        mentions.on_event(&SystemEvent::PointerDown {
            pos: Point::new(8.0, 34.0 + SUGGESTION_ROW_HEIGHT_FOR_TEST * 2.0 + 1.0),
            button: MouseButton::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(mentions.value(), "@Grace ");
}

#[test]
fn mentions_focus_out_preserves_uncommitted_query_text() {
    let mut mentions = mentions();
    let _ = mentions.on_event(&SystemEvent::TextInput {
        text: "@Ada".to_owned(),
    });
    let _ = mentions.on_event(&SystemEvent::FocusOut);
    assert_eq!(mentions.value(), "@Ada");
    assert!(!mentions.is_suggesting());
}

const SUGGESTION_ROW_HEIGHT_FOR_TEST: f32 = 32.0;
