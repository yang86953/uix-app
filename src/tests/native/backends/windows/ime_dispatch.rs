use crate::tests::common::*;
use crate::native::backends::windows::text_input::WindowsImeState;
use crate::native::traits::event::{UiEvent, UiEventType};
use crate::native::backends::windows::ime_dispatch::*;
use crate::native::traits::event::UiEventPayload;

fn payload_text(event: &UiEvent) -> Option<&str> {
    match &event.payload {
        UiEventPayload::ImeComposition(data) => Some(data.text.as_str()),
        UiEventPayload::TextInput(data) => Some(data.text.as_str()),
        _ => None,
    }
}

#[test]
fn compstr_emits_start_and_update() {
    let mut state = WindowsImeState::default();
    let (handled, events) = ime_composition_events(
        &mut state,
        ImmStringRead::Skipped,
        ImmStringRead::Text("zh".into()),
    );
    assert!(handled);
    assert!(state.composition_active());
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].type_, UiEventType::ImeCompositionStart);
    assert_eq!(events[1].type_, UiEventType::ImeCompositionUpdate);
    assert_eq!(payload_text(&events[1]), Some("zh"));
}

#[test]
fn resultstr_emits_end_and_text_input() {
    let mut state = WindowsImeState::default();
    assert!(state.begin_composition());
    let (handled, events) = ime_composition_events(
        &mut state,
        ImmStringRead::Text("中".into()),
        ImmStringRead::Skipped,
    );
    assert!(handled);
    assert!(!state.composition_active());
    assert_eq!(events.len(), 2);
    assert_eq!(events[0].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(payload_text(&events[0]), Some("中"));
    assert_eq!(events[1].type_, UiEventType::TextInput);
    assert_eq!(payload_text(&events[1]), Some("中"));
}

#[test]
fn result_before_comp_in_same_message() {
    let mut state = WindowsImeState::default();
    assert!(state.begin_composition());
    let (handled, events) = ime_composition_events(
        &mut state,
        ImmStringRead::Text("中".into()),
        ImmStringRead::Text("pin".into()),
    );
    assert!(handled);
    assert!(state.composition_active());
    assert_eq!(events[0].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(events[1].type_, UiEventType::TextInput);
    assert_eq!(events[2].type_, UiEventType::ImeCompositionStart);
    assert_eq!(events[3].type_, UiEventType::ImeCompositionUpdate);
    assert_eq!(payload_text(&events[3]), Some("pin"));
}

#[test]
fn empty_comp_while_active_still_updates() {
    let mut state = WindowsImeState::default();
    assert!(state.begin_composition());
    let (handled, events) = ime_composition_events(
        &mut state,
        ImmStringRead::Skipped,
        ImmStringRead::Text(String::new()),
    );
    assert!(handled);
    assert!(state.composition_active());
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].type_, UiEventType::ImeCompositionUpdate);
    assert_eq!(payload_text(&events[0]), Some(""));
}

#[test]
fn empty_comp_when_inactive_is_handled_but_silent() {
    let mut state = WindowsImeState::default();
    let (handled, events) = ime_composition_events(
        &mut state,
        ImmStringRead::Skipped,
        ImmStringRead::Text(String::new()),
    );
    assert!(handled);
    assert!(!state.composition_active());
    assert!(events.is_empty());
}

#[test]
fn no_data_reads_do_not_mark_handled() {
    let mut state = WindowsImeState::default();
    let (handled, events) =
        ime_composition_events(&mut state, ImmStringRead::NoData, ImmStringRead::NoData);
    assert!(!handled);
    assert!(events.is_empty());
}

#[test]
fn start_and_end_helpers_are_idempotent() {
    let mut state = WindowsImeState::default();
    assert!(ime_start_composition_event(&mut state).is_some());
    assert!(ime_start_composition_event(&mut state).is_none());
    assert!(ime_end_composition_event(&mut state).is_some());
    assert!(ime_end_composition_event(&mut state).is_none());
}

#[test]
fn from_flagged_result_maps_branches() {
    assert_eq!(
        ImmStringRead::from_flagged_result(false, Ok(Some("x".into()))),
        ImmStringRead::Skipped
    );
    assert_eq!(
        ImmStringRead::from_flagged_result(true, Ok(None)),
        ImmStringRead::NoData
    );
    assert_eq!(
        ImmStringRead::from_flagged_result(true, Ok(Some("x".into()))),
        ImmStringRead::Text("x".into())
    );
    assert_eq!(
        ImmStringRead::from_flagged_result(
            true,
            Err(crate::core::Error::new(
                crate::core::Errc::PlatformError,
                "boom",
            ))
        ),
        ImmStringRead::Skipped
    );
}
