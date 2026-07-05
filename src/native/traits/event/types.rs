// ============================================================================
// platform/event.rs — 平台无关事件类型
// ============================================================================

use crate::core::geometry::Point;
use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};

// ════════════════════════════════════════════════════════════════════════════
// 事件类型枚举
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum UiEventType {
    Unknown,
    WindowClose,
    WindowResize,
    WindowMinimize,
    WindowMaximize,
    WindowRestore,
    WindowFocus,
    WindowBlur,
    PointerDown,
    PointerUp,
    PointerMove,
    Wheel,
    KeyDown,
    KeyUp,
    TextInput,
    Timer,
    FileDrop,
}

// ════════════════════════════════════════════════════════════════════════════
// 事件载荷结构体
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct KeyEventData {
    pub key: KeyCode,
    pub mods: KeyMod,
}

impl Default for KeyEventData {
    fn default() -> Self {
        Self {
            key: KeyCode::Unknown,
            mods: KeyMod::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerButtonEventData {
    pub pos: Point,
    pub btn: MouseButton,
    pub mods: KeyMod,
}

impl Default for PointerButtonEventData {
    fn default() -> Self {
        Self {
            pos: Point::default(),
            btn: MouseButton::None,
            mods: KeyMod::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerMoveEventData {
    pub pos: Point,
    pub mods: KeyMod,
}

impl Default for PointerMoveEventData {
    fn default() -> Self {
        Self {
            pos: Point::default(),
            mods: KeyMod::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WheelData {
    pub pos: Point,
    pub delta_x: f32,
    pub delta_y: f32,
    pub mods: KeyMod,
}

impl Default for WheelData {
    fn default() -> Self {
        Self {
            pos: Point::default(),
            delta_x: 0.0,
            delta_y: 0.0,
            mods: KeyMod::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ResizeData {
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimerEventData {
    pub timer_id: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TextInputData {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileDropData {
    pub files: Vec<String>,
    pub position: Point,
}

// ════════════════════════════════════════════════════════════════════════════
// UiEventPayload — 事件载荷枚举
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq, Default)]
pub enum UiEventPayload {
    #[default]
    None,
    Key(KeyEventData),
    PointerButton(PointerButtonEventData),
    PointerMove(PointerMoveEventData),
    Wheel(WheelData),
    Resize(ResizeData),
    Timer(TimerEventData),
    TextInput(TextInputData),
    FileDrop(FileDropData),
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent — 事件容器
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct UiEvent {
    pub type_: UiEventType,
    pub payload: UiEventPayload,
}

impl UiEvent {
    pub fn new(type_: UiEventType, payload: UiEventPayload) -> Self {
        Self { type_, payload }
    }

    // -- 工厂方法 ------------------------------------------------

    pub fn close() -> Self {
        Self {
            type_: UiEventType::WindowClose,
            payload: UiEventPayload::None,
        }
    }

    pub fn pointer_down(pos: Point, btn: MouseButton) -> Self {
        Self {
            type_: UiEventType::PointerDown,
            payload: UiEventPayload::PointerButton(PointerButtonEventData {
                pos,
                btn,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn pointer_up(pos: Point, btn: MouseButton) -> Self {
        Self {
            type_: UiEventType::PointerUp,
            payload: UiEventPayload::PointerButton(PointerButtonEventData {
                pos,
                btn,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn pointer_move(pos: Point) -> Self {
        Self {
            type_: UiEventType::PointerMove,
            payload: UiEventPayload::PointerMove(PointerMoveEventData {
                pos,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn wheel(pos: Point, delta_x: f32, delta_y: f32, mods: KeyMod) -> Self {
        Self {
            type_: UiEventType::Wheel,
            payload: UiEventPayload::Wheel(WheelData {
                pos,
                delta_x,
                delta_y,
                mods,
            }),
        }
    }

    pub fn key_down(key: KeyCode, mods: KeyMod) -> Self {
        Self {
            type_: UiEventType::KeyDown,
            payload: UiEventPayload::Key(KeyEventData { key, mods }),
        }
    }

    pub fn key_up(key: KeyCode, mods: KeyMod) -> Self {
        Self {
            type_: UiEventType::KeyUp,
            payload: UiEventPayload::Key(KeyEventData { key, mods }),
        }
    }

    pub fn text_input(text: impl Into<String>) -> Self {
        Self {
            type_: UiEventType::TextInput,
            payload: UiEventPayload::TextInput(TextInputData { text: text.into() }),
        }
    }

    pub fn timer(timer_id: u32) -> Self {
        Self {
            type_: UiEventType::Timer,
            payload: UiEventPayload::Timer(TimerEventData { timer_id }),
        }
    }

    pub fn resize(width: i32, height: i32) -> Self {
        Self {
            type_: UiEventType::WindowResize,
            payload: UiEventPayload::Resize(ResizeData { width, height }),
        }
    }

    pub fn file_drop(files: Vec<String>, position: Point) -> Self {
        Self {
            type_: UiEventType::FileDrop,
            payload: UiEventPayload::FileDrop(FileDropData { files, position }),
        }
    }
}
