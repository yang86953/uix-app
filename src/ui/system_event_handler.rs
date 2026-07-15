use crate::ui::{EventResult, SystemEvent};

pub(crate) type SystemEventHandler = Box<dyn FnMut(&SystemEvent) -> EventResult + 'static>;

pub(crate) struct SystemEventHandlerRegistration {
    handler: SystemEventHandler,
    capture_phase: bool,
    continuous_pointer_move: bool,
}

impl SystemEventHandlerRegistration {
    pub(crate) fn new(handler: SystemEventHandler) -> Self {
        Self {
            handler,
            capture_phase: false,
            continuous_pointer_move: false,
        }
    }

    pub(crate) fn handle(&mut self, event: &SystemEvent) -> EventResult {
        (self.handler)(event)
    }

    pub(crate) fn wants_capture_phase(&self) -> bool {
        self.capture_phase
    }

    pub(crate) fn wants_continuous_pointer_move(&self) -> bool {
        self.continuous_pointer_move
    }
}
