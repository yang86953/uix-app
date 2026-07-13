use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::native::traits::event::UiEvent;

/// Tracks whether an IME composition session is in progress.
#[derive(Debug, Clone, Default)]
pub struct ImeCompositionState {
    pub active: bool,
}

pub fn on_marked_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let Ok(mut queue) = events.lock() else {
        return;
    };
    if !state.active {
        queue.push_back(UiEvent::ime_composition_start());
        state.active = true;
    }
    queue.push_back(UiEvent::ime_composition_update(text));
}

pub fn on_committed_text(
    events: &Arc<Mutex<VecDeque<UiEvent>>>,
    state: &mut ImeCompositionState,
    text: &str,
) {
    if text.is_empty() {
        return;
    }
    let Ok(mut queue) = events.lock() else {
        return;
    };
    if state.active {
        queue.push_back(UiEvent::ime_composition_end(text));
        state.active = false;
    }
    queue.push_back(UiEvent::text_input(text));
}

pub fn on_unmark_text(events: &Arc<Mutex<VecDeque<UiEvent>>>, state: &mut ImeCompositionState) {
    if !state.active {
        return;
    }
    let Ok(mut queue) = events.lock() else {
        return;
    };
    queue.push_back(UiEvent::ime_composition_end(""));
    state.active = false;
}

