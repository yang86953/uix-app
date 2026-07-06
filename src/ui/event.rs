//! 事件模型：SystemEvent / SemanticEvent / HandlerTable。

use std::any::{Any, TypeId};
use std::collections::HashMap;

use crate::core::Point;

use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::widget::{EventResult, WidgetId};

/// 应用边界后的系统事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemEventKind {
    PointerDown,
    PointerUp,
    PointerMove,
    Wheel,
    KeyDown,
    KeyUp,
    TextInput,
    FocusIn,
    FocusOut,
    PointerEnter,
    PointerLeave,
    Resize,
    ThemeChanged,
    LocaleChanged,
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    WindowFocus,
    WindowBlur,
    Timer,
    FileDrop,
    DragStart,
    DragMove,
    DragEnd,
}

#[derive(Debug, Clone)]
pub enum SystemEvent {
    PointerDown {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
    PointerUp {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
    PointerMove {
        pos: Point,
        mods: KeyMod,
    },
    Wheel {
        pos: Point,
        delta: Point,
    },
    KeyDown {
        key: KeyCode,
        mods: KeyMod,
    },
    KeyUp {
        key: KeyCode,
        mods: KeyMod,
    },
    TextInput {
        text: String,
    },
    FocusIn,
    FocusOut,
    PointerEnter,
    PointerLeave,
    ThemeChanged {
        is_dark: bool,
    },
    LocaleChanged {
        locale: String,
    },
    Resize {
        width: f32,
        height: f32,
    },
    WindowMaximize,
    WindowMinimize,
    WindowRestore,
    WindowFocus,
    WindowBlur,
    Timer {
        id: u32,
    },
    FileDrop {
        files: Vec<String>,
        position: Point,
    },
    DragStart {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
    DragMove {
        pos: Point,
        delta: Point,
        mods: KeyMod,
    },
    DragEnd {
        pos: Point,
        button: MouseButton,
        mods: KeyMod,
    },
}

impl SystemEvent {
    pub fn kind(&self) -> SystemEventKind {
        match self {
            SystemEvent::PointerDown { .. } => SystemEventKind::PointerDown,
            SystemEvent::PointerUp { .. } => SystemEventKind::PointerUp,
            SystemEvent::PointerMove { .. } => SystemEventKind::PointerMove,
            SystemEvent::Wheel { .. } => SystemEventKind::Wheel,
            SystemEvent::KeyDown { .. } => SystemEventKind::KeyDown,
            SystemEvent::KeyUp { .. } => SystemEventKind::KeyUp,
            SystemEvent::TextInput { .. } => SystemEventKind::TextInput,
            SystemEvent::FocusIn => SystemEventKind::FocusIn,
            SystemEvent::FocusOut => SystemEventKind::FocusOut,
            SystemEvent::PointerEnter => SystemEventKind::PointerEnter,
            SystemEvent::PointerLeave => SystemEventKind::PointerLeave,
            SystemEvent::ThemeChanged { .. } => SystemEventKind::ThemeChanged,
            SystemEvent::LocaleChanged { .. } => SystemEventKind::LocaleChanged,
            SystemEvent::Resize { .. } => SystemEventKind::Resize,
            SystemEvent::WindowMaximize => SystemEventKind::WindowMaximize,
            SystemEvent::WindowMinimize => SystemEventKind::WindowMinimize,
            SystemEvent::WindowRestore => SystemEventKind::WindowRestore,
            SystemEvent::WindowFocus => SystemEventKind::WindowFocus,
            SystemEvent::WindowBlur => SystemEventKind::WindowBlur,
            SystemEvent::Timer { .. } => SystemEventKind::Timer,
            SystemEvent::FileDrop { .. } => SystemEventKind::FileDrop,
            SystemEvent::DragStart { .. } => SystemEventKind::DragStart,
            SystemEvent::DragMove { .. } => SystemEventKind::DragMove,
            SystemEvent::DragEnd { .. } => SystemEventKind::DragEnd,
        }
    }
}

/// 内置语义事件种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SemanticKind {
    Click,
    Change,
    Submit,
    FileDrop,
    ContextMenu,
    Copy,
    Cut,
    Paste,
    TextInput,
    Custom(TypeId),
}

/// Click 语义载荷。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ClickEvent {
    pub button: MouseButton,
    pub pos: Point,
    pub modifiers: KeyMod,
}

/// 语义事件载荷。
pub enum SemanticPayload {
    None,
    Click(ClickEvent),
    Text(String),
    FileDrop { files: Vec<String>, position: Point },
    Custom(Box<dyn Any>),
}

/// 语义事件。默认冒泡；handler 可 stop / preventDefault。
pub struct SemanticEvent {
    pub kind: SemanticKind,
    pub target: WidgetId,
    pub current_target: WidgetId,
    pub payload: SemanticPayload,
    propagation_stopped: bool,
    default_prevented: bool,
}

impl SemanticEvent {
    pub fn new(kind: SemanticKind, target: WidgetId, payload: SemanticPayload) -> Self {
        Self {
            kind,
            target,
            current_target: target,
            payload,
            propagation_stopped: false,
            default_prevented: false,
        }
    }

    pub fn click(target: WidgetId, payload: ClickEvent) -> Self {
        Self::new(SemanticKind::Click, target, SemanticPayload::Click(payload))
    }

    pub fn context_menu(target: WidgetId, payload: ClickEvent) -> Self {
        Self::new(
            SemanticKind::ContextMenu,
            target,
            SemanticPayload::Click(payload),
        )
    }

    pub fn text_input(target: WidgetId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::TextInput,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    pub fn change(target: WidgetId, value: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Change,
            target,
            SemanticPayload::Text(value.into()),
        )
    }

    pub fn submit(target: WidgetId, value: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Submit,
            target,
            SemanticPayload::Text(value.into()),
        )
    }

    pub fn file_drop(target: WidgetId, files: Vec<String>, position: Point) -> Self {
        Self::new(
            SemanticKind::FileDrop,
            target,
            SemanticPayload::FileDrop { files, position },
        )
    }

    pub fn custom<T: Any>(target: WidgetId, payload: T) -> Self {
        Self::new(
            SemanticKind::Custom(TypeId::of::<T>()),
            target,
            SemanticPayload::Custom(Box::new(payload)),
        )
    }

    pub fn stop_propagation(&mut self) {
        self.propagation_stopped = true;
    }

    pub fn prevent_default(&mut self) {
        self.default_prevented = true;
    }

    pub fn propagation_stopped(&self) -> bool {
        self.propagation_stopped
    }

    pub fn default_prevented(&self) -> bool {
        self.default_prevented
    }

    pub fn click_payload(&self) -> Option<&ClickEvent> {
        match &self.payload {
            SemanticPayload::Click(payload) => Some(payload),
            _ => None,
        }
    }

    pub fn text_payload(&self) -> Option<&str> {
        match &self.payload {
            SemanticPayload::Text(text) => Some(text),
            _ => None,
        }
    }

    pub fn file_drop_payload(&self) -> Option<(&[String], Point)> {
        match &self.payload {
            SemanticPayload::FileDrop { files, position } => Some((files, *position)),
            _ => None,
        }
    }

    pub fn custom_payload<T: Any>(&self) -> Option<&T> {
        match &self.payload {
            SemanticPayload::Custom(payload) => payload.downcast_ref::<T>(),
            _ => None,
        }
    }
}

#[derive(Default)]
pub struct HandlerOptions {
    pub once: bool,
    pub when: Option<Box<dyn Fn(&SemanticEvent) -> bool + 'static>>,
}

impl HandlerOptions {
    pub fn once() -> Self {
        Self {
            once: true,
            when: None,
        }
    }

    pub fn when(f: impl Fn(&SemanticEvent) -> bool + 'static) -> Self {
        Self {
            once: false,
            when: Some(Box::new(f)),
        }
    }
}

pub type SemanticHandler = Box<dyn FnMut(&mut SemanticEvent) + 'static>;

pub struct HandlerRegistration {
    pub kind: SemanticKind,
    pub options: HandlerOptions,
    pub handler: SemanticHandler,
}

impl HandlerRegistration {
    pub fn new(kind: SemanticKind, handler: SemanticHandler) -> Self {
        Self {
            kind,
            options: HandlerOptions::default(),
            handler,
        }
    }

    pub fn with_options(
        kind: SemanticKind,
        options: HandlerOptions,
        handler: SemanticHandler,
    ) -> Self {
        Self {
            kind,
            options,
            handler,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerId(usize);

struct HandlerEntry {
    id: HandlerId,
    kind: SemanticKind,
    options: HandlerOptions,
    handler: SemanticHandler,
}

#[derive(Default)]
pub struct HandlerTable {
    handlers: HashMap<WidgetId, Vec<HandlerEntry>>,
    next_id: usize,
}

impl HandlerTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        component: WidgetId,
        registration: HandlerRegistration,
    ) -> HandlerId {
        let id = HandlerId(self.next_id);
        self.next_id += 1;
        self.handlers
            .entry(component)
            .or_default()
            .push(HandlerEntry {
                id,
                kind: registration.kind,
                options: registration.options,
                handler: registration.handler,
            });
        id
    }

    pub fn on(
        &mut self,
        component: WidgetId,
        kind: SemanticKind,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> HandlerId {
        self.register(component, HandlerRegistration::new(kind, Box::new(handler)))
    }

    pub fn on_click(
        &mut self,
        component: WidgetId,
        mut handler: impl FnMut(&ClickEvent) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::Click, move |event| {
            if let Some(payload) = event.click_payload() {
                handler(payload);
            }
        })
    }

    pub fn on_text_input(
        &mut self,
        component: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::TextInput, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    pub fn on_change(
        &mut self,
        component: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::Change, move |event| {
            if let Some(value) = event.text_payload() {
                handler(value);
            }
        })
    }

    pub fn on_submit(
        &mut self,
        component: WidgetId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::Submit, move |event| {
            if let Some(value) = event.text_payload() {
                handler(value);
            }
        })
    }

    pub fn on_file_drop(
        &mut self,
        component: WidgetId,
        mut handler: impl FnMut(&[String], Point) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::FileDrop, move |event| {
            if let Some((files, position)) = event.file_drop_payload() {
                handler(files, position);
            }
        })
    }

    pub fn remove(&mut self, component: WidgetId, handler_id: HandlerId) {
        if let Some(entries) = self.handlers.get_mut(&component) {
            entries.retain(|entry| entry.id != handler_id);
        }
    }

    pub fn clear_component(&mut self, component: WidgetId) {
        self.handlers.remove(&component);
    }

    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    pub fn dispatch_path(&mut self, path: &[WidgetId], event: &mut SemanticEvent) -> EventResult {
        let mut handled = false;
        for &component in path {
            event.current_target = component;
            if self.dispatch_component(component, event) {
                handled = true;
            }
            if event.propagation_stopped() {
                break;
            }
        }
        if handled {
            EventResult::Handled
        } else {
            EventResult::NotHandled
        }
    }

    fn dispatch_component(&mut self, component: WidgetId, event: &mut SemanticEvent) -> bool {
        let Some(entries) = self.handlers.get_mut(&component) else {
            return false;
        };

        let mut handled = false;
        let mut idx = 0;
        while idx < entries.len() {
            let matches = entries[idx].kind == event.kind;
            let enabled = entries[idx]
                .options
                .when
                .as_ref()
                .map(|predicate| predicate(event))
                .unwrap_or(true);

            if matches && enabled {
                handled = true;
                (entries[idx].handler)(event);
                if entries[idx].options.once {
                    entries.remove(idx);
                } else {
                    idx += 1;
                }
                if event.propagation_stopped() {
                    break;
                }
            } else {
                idx += 1;
            }
        }
        handled
    }
}

#[macro_export]
macro_rules! register_semantic {
    ($payload:ty) => {
        $crate::ui::event::SemanticKind::Custom(std::any::TypeId::of::<$payload>())
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::rc::Rc;

    #[test]
    fn handler_table_bubbles_until_stopped() {
        let child = 2;
        let parent = 1;
        let called_child = Rc::new(Cell::new(false));
        let called_parent = Rc::new(Cell::new(false));

        let mut table = HandlerTable::new();
        {
            let called_child = called_child.clone();
            table.on(child, SemanticKind::Click, move |event| {
                called_child.set(true);
                event.stop_propagation();
            });
        }
        {
            let called_parent = called_parent.clone();
            table.on(parent, SemanticKind::Click, move |_| {
                called_parent.set(true);
            });
        }

        let mut event = SemanticEvent::click(
            child,
            ClickEvent {
                button: MouseButton::Left,
                pos: Point::zero(),
                modifiers: KeyMod::NONE,
            },
        );
        let result = table.dispatch_path(&[child, parent], &mut event);

        assert_eq!(result, EventResult::Handled);
        assert!(called_child.get());
        assert!(!called_parent.get());
    }

    #[test]
    fn handler_table_once_removes_after_first_dispatch() {
        let id = 1;
        let calls = Rc::new(Cell::new(0));
        let mut table = HandlerTable::new();
        let calls_for_handler = calls.clone();
        table.register(
            id,
            HandlerRegistration::with_options(
                SemanticKind::Click,
                HandlerOptions::once(),
                Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
            ),
        );

        for _ in 0..2 {
            let mut event = SemanticEvent::click(
                id,
                ClickEvent {
                    button: MouseButton::Left,
                    pos: Point::zero(),
                    modifiers: KeyMod::NONE,
                },
            );
            let _ = table.dispatch_path(&[id], &mut event);
        }

        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn handler_table_when_is_evaluated_per_dispatch() {
        let id = 1;
        let enabled = Rc::new(Cell::new(false));
        let calls = Rc::new(Cell::new(0));
        let mut table = HandlerTable::new();
        let enabled_for_predicate = enabled.clone();
        let calls_for_handler = calls.clone();
        table.register(
            id,
            HandlerRegistration::with_options(
                SemanticKind::Click,
                HandlerOptions::when(move |_| enabled_for_predicate.get()),
                Box::new(move |_| calls_for_handler.set(calls_for_handler.get() + 1)),
            ),
        );

        let mut event = SemanticEvent::click(
            id,
            ClickEvent {
                button: MouseButton::Left,
                pos: Point::zero(),
                modifiers: KeyMod::NONE,
            },
        );
        let _ = table.dispatch_path(&[id], &mut event);
        enabled.set(true);
        let mut event = SemanticEvent::click(
            id,
            ClickEvent {
                button: MouseButton::Left,
                pos: Point::zero(),
                modifiers: KeyMod::NONE,
            },
        );
        let _ = table.dispatch_path(&[id], &mut event);

        assert_eq!(calls.get(), 1);
    }
}
