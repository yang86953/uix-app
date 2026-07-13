use crate::core::WindowId;
use crate::native::shared::ime_events::*;
use crate::native::traits::event::UiEvent;
use crate::native::traits::event::{UiEventPayload, UiEventType};
use crate::tests::common::*;

fn queue(events: &Arc<Mutex<VecDeque<UiEvent>>>) -> std::sync::MutexGuard<'_, VecDeque<UiEvent>> {
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

#[test]
fn targeted_composition_and_commit_keep_one_window_id() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();
    let window_id = WindowId::new(42);

    on_marked_text_for_window(&events, &mut state, "zh", window_id);
    on_committed_text_for_window(&events, &mut state, "a", window_id);

    let q = queue(&events);
    assert_eq!(q.len(), 4);
    assert!(q.iter().all(|event| event.window_id == Some(window_id)));
    assert_eq!(q[0].type_, UiEventType::ImeCompositionStart);
    assert_eq!(q[1].type_, UiEventType::ImeCompositionUpdate);
    assert_eq!(q[2].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(q[3].type_, UiEventType::TextInput);
}

#[test]
fn targeted_unmark_keeps_the_composition_owner() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();
    let window_id = WindowId::new(7);

    on_marked_text_for_window(&events, &mut state, "x", window_id);
    on_unmark_text_for_window(&events, &mut state, window_id);

    let q = queue(&events);
    assert_eq!(q.len(), 3);
    assert_eq!(q[2].window_id, Some(window_id));
    assert_eq!(q[2].type_, UiEventType::ImeCompositionEnd);
}

#[test]
fn pending_batch_applies_preedit_then_commit_at_explicit_boundaries() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();
    let mut batch = PendingImeBatch::default();
    let window_id = WindowId::new(9);

    batch.set_preedit(Some("zh".to_string()));
    batch.apply_for_window(&events, &mut state, window_id);
    batch.set_commit(Some("a".to_string()));
    batch.apply_for_window(&events, &mut state, window_id);

    assert!(!state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 4);
    assert!(q.iter().all(|event| event.window_id == Some(window_id)));
    assert_eq!(q[0].type_, UiEventType::ImeCompositionStart);
    assert_eq!(q[1].type_, UiEventType::ImeCompositionUpdate);
    assert_eq!(q[2].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(q[3].type_, UiEventType::TextInput);
}

#[test]
fn one_batch_can_commit_old_composition_and_start_new_preedit() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();
    let mut batch = PendingImeBatch::default();
    let window_id = WindowId::new(11);
    on_marked_text_for_window(&events, &mut state, "old", window_id);

    batch.set_commit(Some("a".to_string()));
    batch.set_preedit(Some("new".to_string()));
    batch.apply_for_window(&events, &mut state, window_id);

    assert!(state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 6);
    assert_eq!(q[2].type_, UiEventType::ImeCompositionEnd);
    assert_eq!(q[3].type_, UiEventType::TextInput);
    assert_eq!(q[4].type_, UiEventType::ImeCompositionStart);
    assert_eq!(q[5].type_, UiEventType::ImeCompositionUpdate);
    assert!(q.iter().all(|event| event.window_id == Some(window_id)));
}

#[test]
fn empty_pending_batch_clears_an_existing_composition() {
    let events = Arc::new(Mutex::new(VecDeque::new()));
    let mut state = ImeCompositionState::default();
    let mut batch = PendingImeBatch::default();
    let window_id = WindowId::new(13);
    on_marked_text_for_window(&events, &mut state, "active", window_id);
    events.lock().unwrap().clear();

    batch.apply_for_window(&events, &mut state, window_id);

    assert!(!state.active);
    let q = queue(&events);
    assert_eq!(q.len(), 1);
    assert_eq!(q[0].window_id, Some(window_id));
    assert_eq!(q[0].type_, UiEventType::ImeCompositionEnd);
}
