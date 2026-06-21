use crate::widget::{WidgetEvent, EventResult};

/// Manages event handling chain for a widget.
#[derive(Default)]
pub struct EventManager {
    handlers: Vec<Box<dyn FnMut(&WidgetEvent) -> EventResult>>,
}

impl EventManager {
    pub fn new() -> Self { Self::default() }

    pub fn add_handler<F: FnMut(&WidgetEvent) -> EventResult + 'static>(&mut self, f: F) {
        self.handlers.push(Box::new(f));
    }

    pub fn dispatch(&mut self, event: &WidgetEvent) -> EventResult {
        for handler in &mut self.handlers {
            if handler(event) == EventResult::Handled {
                return EventResult::Handled;
            }
        }
        EventResult::NotHandled
    }

    pub fn clear(&mut self) { self.handlers.clear(); }
    pub fn len(&self) -> usize { self.handlers.len() }
    pub fn is_empty(&self) -> bool { self.handlers.is_empty() }
}
