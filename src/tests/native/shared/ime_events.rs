use crate::tests::common::*;
use crate::native::traits::event::UiEvent;
use crate::native::shared::ime_events::*;
use crate::native::traits::event::{UiEventPayload, UiEventType};

fn queue(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
) -> std::sync::MutexGuard<'_, VecDeque<UiEvent>> {
    events.lock().unwrap()
}

#[test]
fn marked_text_starts_session_and_queues_update() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();

    on_marked_text(&events, &mut state, "zh");

    assert!(state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 2);
    assert_eq!(q[0].type_, UiEventType::ImeCompositionStart);
    assert_eq!(q[1].type_, UiEventType::ImeCompositionUpdate);
    if let UiEventPayload::ImeComposition(ref data) = q[1].payload {
        assert_eq!(data.text, "zh");
    } else {
        panic!("expected ime composition payload");
    }
}

#[test]
fn subsequent_marked_text_only_updates() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();

    on_marked_text(&events, &mut state, "z");
    on_marked_text(&events, &mut state, "zh");

    let q = queue(&events);
    assert_eq!(q.len(), 3);
    assert_eq!(q[0].type_, UiEventType::ImeCompositionStart);
    assert_eq!(q[1].type_, UiEventType::ImeCompositionUpdate);
    assert_eq!(q[2].type_, UiEventType::ImeCompositionUpdate);
}

#[test]
fn committed_text_ends_session_and_queues_text_input() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();

    on_marked_text(&events, &mut state, "zh");
    on_committed_text(&events, &mut state, "中");

    assert!(!state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 4);
    assert_eq!(q[2].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(q[3].type_, UiEventType::TextInput);
    if let UiEventPayload::TextInput(ref data) = q[3].payload {
        assert_eq!(data.text, "中");
    } else {
        panic!("expected text input payload");
    }
}

#[test]
fn direct_commit_without_marked_text_queues_text_input_only() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();

    on_committed_text(&events, &mut state, "a");

    assert!(!state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].type_, UiEventType::TextInput);
}

#[test]
fn unmark_text_ends_session_with_empty_commit() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();

    on_marked_text(&events, &mut state, "zh");
    on_unmark_text(&events, &mut state);

    assert!(!state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 3);
    assert_eq!(q[2].type_, UiEventType::ImeCompositionEnd);
    if let UiEventPayload::ImeComposition(ref data) = q[2].payload {
        assert_eq!(data.text, "");
    } else {
        panic!("expected ime composition payload");
    }
}
