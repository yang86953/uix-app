use super::{EventResult, SystemEvent};

pub(crate) type SystemEventHandler = Box<dyn FnMut(&SystemEvent) -> EventResult + 'static>;

#[derive(Clone, Copy)]
pub(crate) enum SystemEventFilter {
    Any,
    Pointer,
    Key,
    Focus,
    Scroll,
}

impl SystemEventFilter {
    fn matches(self, event: &SystemEvent) -> bool {
        match self {
            Self::Any => true,
            Self::Pointer => matches!(
                event,
                SystemEvent::PointerDown { .. }
                    | SystemEvent::PointerDoubleClick { .. }
                    | SystemEvent::PointerUp { .. }
                    | SystemEvent::PointerMove { .. }
                    | SystemEvent::PointerEnter
                    | SystemEvent::PointerLeave
            ),
            Self::Key => matches!(
                event,
                SystemEvent::KeyDown { .. } | SystemEvent::KeyUp { .. }
            ),
            Self::Focus => matches!(event, SystemEvent::FocusIn | SystemEvent::FocusOut),
            Self::Scroll => matches!(event, SystemEvent::Wheel { .. }),
        }
    }
}

pub(crate) struct SystemEventHandlerRegistration {
    filter: SystemEventFilter,
    handler: SystemEventHandler,
    capture_phase: bool,
    continuous_pointer_move: bool,
}

impl SystemEventHandlerRegistration {
    pub(crate) fn new(handler: SystemEventHandler) -> Self {
        Self {
            filter: SystemEventFilter::Any,
            handler,
            capture_phase: false,
            continuous_pointer_move: false,
        }
    }

    pub(crate) fn filtered(filter: SystemEventFilter, handler: SystemEventHandler) -> Self {
        Self {
            filter,
            handler,
            capture_phase: false,
            continuous_pointer_move: matches!(filter, SystemEventFilter::Pointer),
        }
    }

    pub(crate) fn capture_phase(mut self) -> Self {
        self.capture_phase = true;
        self
    }

    pub(crate) fn handle(&mut self, event: &SystemEvent) -> Option<EventResult> {
        self.filter.matches(event).then(|| (self.handler)(event))
    }

    pub(crate) fn wants_capture_phase(&self) -> bool {
        self.capture_phase
    }

    pub(crate) fn wants_continuous_pointer_move(&self) -> bool {
        self.continuous_pointer_move
    }
}
