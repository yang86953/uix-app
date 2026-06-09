use crate::graphics::Point;
use crate::ui::widget::{EventResult, WidgetEvent};

/// Callback types for user interactions.
pub type ClickCallback = Box<dyn FnMut(&Point)>;
pub type ChangedCallback = Box<dyn FnMut()>;
pub type SubmitCallback = Box<dyn FnMut()>;

/// Manages mouse/touch interaction callbacks.
#[derive(Default)]
pub struct InteractionManager {
    on_click: Option<ClickCallback>,
    on_changed: Option<ChangedCallback>,
    on_submit: Option<SubmitCallback>,
    hovered: bool,
    pressed: bool,
}


impl InteractionManager {
    pub fn new() -> Self { Self::default() }

    pub fn set_on_click<F: FnMut(&Point) + 'static>(&mut self, f: F) {
        self.on_click = Some(Box::new(f));
    }

    pub fn set_on_changed<F: FnMut() + 'static>(&mut self, f: F) {
        self.on_changed = Some(Box::new(f));
    }

    pub fn set_on_submit<F: FnMut() + 'static>(&mut self, f: F) {
        self.on_submit = Some(Box::new(f));
    }

    pub fn hovered(&self) -> bool { self.hovered }
    pub fn pressed(&self) -> bool { self.pressed }

    pub fn handle_event(&mut self, event: &WidgetEvent) -> EventResult {
        match event {
            WidgetEvent::MouseDown { .. } => {
                self.pressed = true;
                EventResult::Handled
            }
            WidgetEvent::MouseUp { pos, .. } => {
                self.pressed = false;
                if let Some(ref mut cb) = self.on_click {
                    cb(pos);
                }
                EventResult::Handled
            }
            WidgetEvent::HoverEnter => {
                self.hovered = true;
                EventResult::Handled
            }
            WidgetEvent::HoverLeave => {
                self.hovered = false;
                EventResult::Handled
            }
            _ => EventResult::NotHandled,
        }
    }
}
