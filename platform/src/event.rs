// ============================================================================
// platform/event.rs — 平台无关事件类型
// ============================================================================

use uix_core::*;

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
    MouseDown,
    MouseUp,
    MouseMove,
    MouseWheel,
    KeyDown,
    KeyUp,
    KeyPress,
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
pub struct MouseButtonEventData {
    pub pos: Point,
    pub btn: MouseButton,
    pub mods: KeyMod,
}

impl Default for MouseButtonEventData {
    fn default() -> Self {
        Self {
            pos: Point::default(),
            btn: MouseButton::None,
            mods: KeyMod::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseMoveEventData {
    pub pos: Point,
    pub mods: KeyMod,
}

impl Default for MouseMoveEventData {
    fn default() -> Self {
        Self {
            pos: Point::default(),
            mods: KeyMod::NONE,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseWheelData {
    pub pos: Point,
    pub delta_x: f32,
    pub delta_y: f32,
    pub mods: KeyMod,
}

impl Default for MouseWheelData {
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
pub struct KeyPressData {
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
    MouseButton(MouseButtonEventData),
    MouseMove(MouseMoveEventData),
    MouseWheel(MouseWheelData),
    Resize(ResizeData),
    Timer(TimerEventData),
    KeyPress(KeyPressData),
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

    pub fn mouse_down(pos: Point, btn: MouseButton) -> Self {
        Self {
            type_: UiEventType::MouseDown,
            payload: UiEventPayload::MouseButton(MouseButtonEventData {
                pos,
                btn,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn mouse_up(pos: Point, btn: MouseButton) -> Self {
        Self {
            type_: UiEventType::MouseUp,
            payload: UiEventPayload::MouseButton(MouseButtonEventData {
                pos,
                btn,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn mouse_move(pos: Point) -> Self {
        Self {
            type_: UiEventType::MouseMove,
            payload: UiEventPayload::MouseMove(MouseMoveEventData {
                pos,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn mouse_wheel(pos: Point, delta_x: f32, delta_y: f32, mods: KeyMod) -> Self {
        Self {
            type_: UiEventType::MouseWheel,
            payload: UiEventPayload::MouseWheel(MouseWheelData {
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

    pub fn key_press(text: impl Into<String>) -> Self {
        Self {
            type_: UiEventType::KeyPress,
            payload: UiEventPayload::KeyPress(KeyPressData { text: text.into() }),
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
