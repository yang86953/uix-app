use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::core::WindowId;
use crate::native::windowing::event::UiEvent;

fn event_queue(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
) -> std::sync::MutexGuard<'_, VecDeque<UiEvent>> {
    events.lock().unwrap_or_else(|error| error.into_inner())
}

/// Tracks whether an IME composition session is in progress.
#[derive(Debug, Clone, Default)]
pub(crate) struct ImeCompositionState {
    pub(crate) active: bool,
}

/// Accumulates a double-buffered native IME update until its apply boundary.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub(crate) struct PendingImeBatch {
    preedit: Option<String>,
    commit: Option<String>,
}

#[allow(dead_code)]
impl PendingImeBatch {
    pub(crate) fn set_preedit(&mut self, text: Option<String>) {
        self.preedit = Some(text.unwrap_or_default());
    }

    pub(crate) fn set_commit(&mut self, text: Option<String>) {
        self.commit = Some(text.unwrap_or_default());
    }

    pub(crate) fn apply_for_window(
        &mut self,
        events: &Arc<Mutex<VecDeque<UiEvent>>>,
        state: &mut ImeCompositionState,
        window_id: WindowId,
    ) {
        let batch = std::mem::take(self);
        match batch.commit.as_deref() {
            Some(text) if !text.is_empty() => {
                on_committed_text_for_window(events, state, text, window_id);
            }
            Some(_) => on_unmark_text_for_window(events, state, window_id),
            None if batch.preedit.is_none() => {
                on_unmark_text_for_window(events, state, window_id);
            }
            None => {}
        }

        if let Some(preedit) = batch.preedit {
            if preedit.is_empty() {
                on_unmark_text_for_window(events, state, window_id);
            } else {
                on_marked_text_for_window(events, state, &preedit, window_id);
            }
        }
    }
}

// 仅 Windows TSF 路径（tsf_session.rs）使用；macOS/Wayland 使用 *_for_window 变体。
pub(crate) fn on_marked_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
) {
    on_marked_text_targeted(events, state, text, None);
}

pub(crate) fn on_marked_text_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: WindowId,
) {
    on_marked_text_targeted(events, state, text, Some(window_id));
}

fn on_marked_text_targeted(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: Option<WindowId>,
) {
    if text.is_empty() {
        return;
    }
    let mut queue = event_queue(events);
    if !state.active {
        queue.push_back(target(UiEvent::ime_composition_start(), window_id));
        state.active = true;
    }
    queue.push_back(target(UiEvent::ime_composition_update(text), window_id));
}

// 仅 Windows TSF 路径（tsf_session.rs）使用；macOS/Wayland 使用 *_for_window 变体。
pub(crate) fn on_committed_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
) {
    on_committed_text_targeted(events, state, text, None);
}

pub(crate) fn on_committed_text_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: WindowId,
) {
    on_committed_text_targeted(events, state, text, Some(window_id));
}

fn on_committed_text_targeted(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
    window_id: Option<WindowId>,
) {
    if text.is_empty() {
        return;
    }
    let mut queue = event_queue(events);
    if state.active {
        queue.push_back(target(UiEvent::ime_composition_end(text), window_id));
        state.active = false;
    }
    queue.push_back(target(UiEvent::text_input(text), window_id));
}

// 仅 Windows TSF 路径（tsf_session.rs）使用；macOS/Wayland 使用 *_for_window 变体。
pub(crate) fn on_unmark_text(events: &Arc<Mutex<VecDeque<UiEvent>>>, state: &mut ImeCompositionState) {
    on_unmark_text_targeted(events, state, None);
}

pub(crate) fn on_unmark_text_for_window(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    window_id: WindowId,
) {
    on_unmark_text_targeted(events, state, Some(window_id));
}

fn on_unmark_text_targeted(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    window_id: Option<WindowId>,
) {
    if !state.active {
        return;
    }
    let mut queue = event_queue(events);
    queue.push_back(target(UiEvent::ime_composition_end(""), window_id));
    state.active = false;
}

fn target(event: UiEvent, window_id: Option<WindowId>) -> UiEvent {
    match window_id {
        Some(window_id) => event.for_window(window_id),
        None => event,
    }
}
