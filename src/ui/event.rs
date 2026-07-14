//! 事件模型：SystemEvent / SemanticEvent / HandlerTable。

use std::any::{Any, TypeId};
use std::collections::{hash_map::DefaultHasher, HashMap};
use std::hash::{Hash, Hasher};

use crate::core::{ComponentId, Point, WindowId};

use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use crate::ui::widget::EventResult;

/// 由 UI 组件发往事件所属原生窗口的无外观动作。
///
/// 该类型只在 `ui -> app` 边界内流转；公开 API 使用
/// [`WindowControl`](crate::ui::WindowControl)
/// 描述可由应用放入自定义标题栏的控制项。
#[doc(hidden)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    BeginMoveDrag,
    Minimize,
    MaximizeRestore,
    ToggleMaximizeFromTitleBar,
    RequestClose,
}

impl WindowAction {
    /// 原生拖动与标题栏双击属于窗口外壳手势，不应改变客户区内已有键盘焦点。
    pub(crate) const fn preserves_keyboard_focus(self) -> bool {
        matches!(self, Self::BeginMoveDrag | Self::ToggleMaximizeFromTitleBar)
    }
}

/// 应用边界后的系统事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemEventKind {
    PointerDown,
    PointerDoubleClick,
    PointerUp,
    PointerMove,
    Wheel,
    KeyDown,
    KeyUp,
    Copy,
    Cut,
    Paste,
    TextInput,
    ImeCompositionStart,
    ImeCompositionUpdate,
    ImeCompositionEnd,
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
    PointerDoubleClick {
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
    Copy,
    Cut,
    Paste {
        text: String,
    },
    TextInput {
        text: String,
    },
    ImeCompositionStart,
    ImeCompositionUpdate {
        text: String,
    },
    ImeCompositionEnd {
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
            SystemEvent::PointerDoubleClick { .. } => SystemEventKind::PointerDoubleClick,
            SystemEvent::PointerUp { .. } => SystemEventKind::PointerUp,
            SystemEvent::PointerMove { .. } => SystemEventKind::PointerMove,
            SystemEvent::Wheel { .. } => SystemEventKind::Wheel,
            SystemEvent::KeyDown { .. } => SystemEventKind::KeyDown,
            SystemEvent::KeyUp { .. } => SystemEventKind::KeyUp,
            SystemEvent::Copy => SystemEventKind::Copy,
            SystemEvent::Cut => SystemEventKind::Cut,
            SystemEvent::Paste { .. } => SystemEventKind::Paste,
            SystemEvent::TextInput { .. } => SystemEventKind::TextInput,
            SystemEvent::ImeCompositionStart => SystemEventKind::ImeCompositionStart,
            SystemEvent::ImeCompositionUpdate { .. } => SystemEventKind::ImeCompositionUpdate,
            SystemEvent::ImeCompositionEnd { .. } => SystemEventKind::ImeCompositionEnd,
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
    ImeCompositionStart,
    ImeCompositionUpdate,
    ImeCompositionEnd,
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
    Custom(Box<dyn Any + Send>),
}

/// 语义事件。默认冒泡；handler 可 stop / preventDefault。
pub struct SemanticEvent {
    pub kind: SemanticKind,
    pub target: ComponentId,
    pub current_target: ComponentId,
    pub payload: SemanticPayload,
    propagation_stopped: bool,
    default_prevented: bool,
}

impl SemanticEvent {
    pub fn new(kind: SemanticKind, target: ComponentId, payload: SemanticPayload) -> Self {
        Self {
            kind,
            target,
            current_target: target,
            payload,
            propagation_stopped: false,
            default_prevented: false,
        }
    }

    pub fn click(target: ComponentId, payload: ClickEvent) -> Self {
        Self::new(SemanticKind::Click, target, SemanticPayload::Click(payload))
    }

    pub fn context_menu(target: ComponentId, payload: ClickEvent) -> Self {
        Self::new(
            SemanticKind::ContextMenu,
            target,
            SemanticPayload::Click(payload),
        )
    }

    pub fn text_input(target: ComponentId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::TextInput,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    pub fn ime_composition_start(target: ComponentId) -> Self {
        Self::new(
            SemanticKind::ImeCompositionStart,
            target,
            SemanticPayload::None,
        )
    }

    pub fn ime_composition_update(target: ComponentId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::ImeCompositionUpdate,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    pub fn ime_composition_end(target: ComponentId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::ImeCompositionEnd,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    pub fn copy(target: ComponentId) -> Self {
        Self::new(SemanticKind::Copy, target, SemanticPayload::None)
    }

    pub fn cut(target: ComponentId) -> Self {
        Self::new(SemanticKind::Cut, target, SemanticPayload::None)
    }

    pub fn paste(target: ComponentId, text: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Paste,
            target,
            SemanticPayload::Text(text.into()),
        )
    }

    pub fn change(target: ComponentId, value: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Change,
            target,
            SemanticPayload::Text(value.into()),
        )
    }

    pub fn submit(target: ComponentId, value: impl Into<String>) -> Self {
        Self::new(
            SemanticKind::Submit,
            target,
            SemanticPayload::Text(value.into()),
        )
    }

    pub fn file_drop(target: ComponentId, files: Vec<String>, position: Point) -> Self {
        Self::new(
            SemanticKind::FileDrop,
            target,
            SemanticPayload::FileDrop { files, position },
        )
    }

    pub fn custom<T: Any + Send>(target: ComponentId, payload: T) -> Self {
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

type HandlerPredicate = dyn Fn(&SemanticEvent) -> bool + 'static;

#[derive(Default)]
pub struct HandlerOptions {
    pub once: bool,
    pub when: Option<Box<HandlerPredicate>>,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerOptionsSignature {
    pub once: bool,
    pub when: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HandlerSignature {
    pub kind: SemanticKind,
    pub options: HandlerOptionsSignature,
    pub generation: Option<u32>,
    pub capture_fingerprint: Option<u64>,
}

pub struct HandlerRegistration {
    pub kind: SemanticKind,
    pub options: HandlerOptions,
    pub handler: SemanticHandler,
    generation: Option<u32>,
    capture_fingerprints: Vec<u64>,
}

impl HandlerRegistration {
    pub fn new(kind: SemanticKind, handler: SemanticHandler) -> Self {
        Self {
            kind,
            options: HandlerOptions::default(),
            handler,
            generation: None,
            capture_fingerprints: Vec::new(),
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
            generation: None,
            capture_fingerprints: Vec::new(),
        }
    }

    #[allow(dead_code)]
    pub(crate) fn with_generation(mut self, generation: u32) -> Self {
        self.generation = Some(generation);
        self
    }

    #[allow(dead_code)]
    pub(crate) fn with_capture_fingerprint(mut self, capture_fingerprint: u64) -> Self {
        self.capture_fingerprints.push(capture_fingerprint);
        self
    }

    /// Marks this handler as capturing `state` for reconcile-time reuse.
    pub fn with_state_capture<T: Clone + Send + Sync + 'static>(
        self,
        state: &crate::ui::foundation::state::State<T>,
    ) -> Self {
        crate::ui::foundation::state::capture_pending_state_bind(state);
        self.with_capture_fingerprint(state.capture_fingerprint())
    }

    /// Marks this handler as capturing `computed` for reconcile-time reuse.
    pub fn with_computed_capture<T: Clone + Send + Sync + 'static>(
        self,
        computed: &crate::ui::foundation::state::Computed<T>,
    ) -> Self {
        self.with_capture_fingerprint(computed.capture_fingerprint())
    }

    /// Marks this handler as capturing a window-scoped app handle.
    pub fn with_window_capture(self, window_id: WindowId) -> Self {
        self.with_capture_fingerprint(window_capture_fingerprint(window_id))
    }

    pub(crate) fn signature(&self) -> HandlerSignature {
        HandlerSignature {
            kind: self.kind,
            options: self.options.signature(),
            generation: self.generation,
            capture_fingerprint: combined_capture_fingerprint(&self.capture_fingerprints),
        }
    }

    pub(crate) fn authored_signature(&self) -> HandlerSignature {
        let mut signature = self.signature();
        if signature.generation.is_none() && signature.capture_fingerprint.is_some() {
            signature.generation = Some(0);
        }
        signature
    }
}

impl HandlerOptions {
    fn signature(&self) -> HandlerOptionsSignature {
        HandlerOptionsSignature {
            once: self.once,
            when: self.when.is_some(),
        }
    }
}

fn combined_capture_fingerprint(captures: &[u64]) -> Option<u64> {
    match captures {
        [] => None,
        [single] => Some(*single),
        _ => {
            let mut sorted = captures.to_vec();
            sorted.sort_unstable();
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            sorted.hash(&mut hasher);
            Some(hasher.finish())
        }
    }
}

fn window_capture_fingerprint(window_id: WindowId) -> u64 {
    let mut hasher = DefaultHasher::new();
    TypeId::of::<WindowId>().hash(&mut hasher);
    window_id.hash(&mut hasher);
    hasher.finish()
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
    handlers: HashMap<ComponentId, Vec<HandlerEntry>>,
    next_id: usize,
}

impl HandlerTable {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        component: ComponentId,
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
        component: ComponentId,
        kind: SemanticKind,
        handler: impl FnMut(&mut SemanticEvent) + 'static,
    ) -> HandlerId {
        self.register(component, HandlerRegistration::new(kind, Box::new(handler)))
    }

    pub fn on_click(
        &mut self,
        component: ComponentId,
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
        component: ComponentId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::TextInput, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    pub fn on_ime_composition_start(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut() + 'static,
    ) -> HandlerId {
        self.on(
            component,
            SemanticKind::ImeCompositionStart,
            move |_event| {
                handler();
            },
        )
    }

    pub fn on_ime_composition_update(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(
            component,
            SemanticKind::ImeCompositionUpdate,
            move |event| {
                if let Some(text) = event.text_payload() {
                    handler(text);
                }
            },
        )
    }

    pub fn on_ime_composition_end(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::ImeCompositionEnd, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    pub fn on_copy(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut() + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::Copy, move |_event| {
            handler();
        })
    }

    pub fn on_cut(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut() + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::Cut, move |_event| {
            handler();
        })
    }

    pub fn on_paste(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut(&str) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::Paste, move |event| {
            if let Some(text) = event.text_payload() {
                handler(text);
            }
        })
    }

    pub fn on_change(
        &mut self,
        component: ComponentId,
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
        component: ComponentId,
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
        component: ComponentId,
        mut handler: impl FnMut(&[String], Point) + 'static,
    ) -> HandlerId {
        self.on(component, SemanticKind::FileDrop, move |event| {
            if let Some((files, position)) = event.file_drop_payload() {
                handler(files, position);
            }
        })
    }

    pub fn on_custom<T: Any>(
        &mut self,
        component: ComponentId,
        mut handler: impl FnMut(&T) + 'static,
    ) -> HandlerId {
        self.on(
            component,
            SemanticKind::Custom(TypeId::of::<T>()),
            move |event| {
                if let Some(payload) = event.custom_payload::<T>() {
                    handler(payload);
                }
            },
        )
    }

    pub fn remove(&mut self, component: ComponentId, handler_id: HandlerId) {
        if let Some(entries) = self.handlers.get_mut(&component) {
            entries.retain(|entry| entry.id != handler_id);
        }
    }

    pub fn clear_component(&mut self, component: ComponentId) {
        self.handlers.remove(&component);
    }

    pub fn clear(&mut self) {
        self.handlers.clear();
    }

    pub fn dispatch_path(
        &mut self,
        path: &[ComponentId],
        event: &mut SemanticEvent,
    ) -> EventResult {
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

    fn dispatch_component(&mut self, component: ComponentId, event: &mut SemanticEvent) -> bool {
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
