use crate::tests::common::*;
use crate::ui::widgets::AutoComplete;

fn autocomplete() -> AutoComplete {
    AutoComplete::new()
        .placeholder("Name")
        .options(vec!["Ada", "Alan", "Grace"])
}

#[test]
fn autocomplete_accepts_platform_text_and_filters_case_insensitively() {
    let mut autocomplete = autocomplete();
    assert!(autocomplete
        .as_text_input()
        .expect("AutoComplete text input capability")
        .accepts_text_input());

    assert_eq!(
        autocomplete.on_event(&SystemEvent::FocusIn),
        EventResult::Handled
    );
    assert_eq!(
        autocomplete.on_event(&SystemEvent::TextInput {
            text: "AL".to_owned(),
        }),
        EventResult::Handled
    );
    assert_eq!(autocomplete.value(), "AL");
    assert_eq!(autocomplete.filtered_options(), &["Alan"]);
    assert!(autocomplete.is_open());
    assert!(matches!(
        autocomplete.snapshot_fields(),
        SnapshotFields::AutoComplete {
            value,
            open: true,
            ..
        } if value == "AL"
    ));
    let accessibility = autocomplete.snapshot_fields().accessibility();
    assert_eq!(accessibility.state.value_text.as_deref(), Some("AL"));
    assert_eq!(accessibility.state.expanded, Some(true));
}

#[test]
fn autocomplete_backspace_refilters_and_enter_commits_match() {
    let mut autocomplete = autocomplete();
    let _ = autocomplete.on_event(&SystemEvent::TextInput {
        text: "ad".to_owned(),
    });
    let _ = autocomplete.on_event(&SystemEvent::KeyDown {
        key: KeyCode::Backspace,
        mods: KeyMod::NONE,
    });

    assert_eq!(autocomplete.value(), "a");
    assert_eq!(autocomplete.filtered_options(), &["Ada", "Alan", "Grace"]);
    assert_eq!(
        autocomplete.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Enter,
            mods: KeyMod::NONE,
        }),
        EventResult::Handled
    );
    assert_eq!(autocomplete.value(), "Ada");
    assert!(!autocomplete.is_open());
    assert!(autocomplete.is_present());
}

#[test]
fn autocomplete_focus_out_closes_without_consuming_unrelated_keys() {
    let mut autocomplete = autocomplete();
    autocomplete.open();
    assert_eq!(
        autocomplete.on_event(&SystemEvent::KeyDown {
            key: KeyCode::Left,
            mods: KeyMod::NONE,
        }),
        EventResult::NotHandled
    );
    assert_eq!(
        autocomplete.on_event(&SystemEvent::FocusOut),
        EventResult::Handled
    );
    assert!(!autocomplete.is_open());
}
