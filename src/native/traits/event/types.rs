// ============================================================================
// platform/event.rs — 平台无关事件类型
// ============================================================================

use crate::core::geometry::Point;
use crate::core::WindowId;
use crate::native::traits::input::{KeyCode, KeyMod, MouseButton};
use std::time::Instant;

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
    FrameOpportunity,
    PointerDown,
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
    Timer,
    FileDrop,
    ThemeChanged,
    LocaleChanged,
    WindowShow,
    WindowHide,
    WindowOcclusionChanged,
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
pub struct ImeCompositionData {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ClipboardData {
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct FileDropData {
    pub files: Vec<String>,
    pub position: Point,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThemeChangeData {
    pub is_dark: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct LocaleChangeData {
    pub locale: String,
}

/// Identifies one outstanding frame request within a surface generation.
///
/// Surface generation alone is insufficient: a fallback may consume one
/// request while its native callback is still in flight, and that callback
/// must not consume the next request on the same surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FrameRequestToken {
    pub surface_generation: u64,
    pub request_id: u64,
}

impl FrameRequestToken {
    pub const fn new(surface_generation: u64, request_id: u64) -> Self {
        Self {
            surface_generation,
            request_id,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameOpportunityData {
    pub token: FrameRequestToken,
    pub frame_time: Instant,
    pub target_present_time: Option<Instant>,
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
    ImeComposition(ImeCompositionData),
    Clipboard(ClipboardData),
    FileDrop(FileDropData),
    ThemeChanged(ThemeChangeData),
    LocaleChanged(LocaleChangeData),
    FrameOpportunity(FrameOpportunityData),
}

// ════════════════════════════════════════════════════════════════════════════
// UiEvent — 事件容器
// ════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, PartialEq)]
pub struct UiEvent {
    pub window_id: Option<WindowId>,
    pub type_: UiEventType,
    pub payload: UiEventPayload,
}

impl UiEvent {
    pub fn new(type_: UiEventType, payload: UiEventPayload) -> Self {
        Self {
            window_id: None,
            type_,
            payload,
        }
    }

    pub fn for_window(mut self, window_id: WindowId) -> Self {
        self.window_id = Some(window_id);
        self
    }

    // -- 工厂方法 ------------------------------------------------

    pub fn close() -> Self {
        Self {
            window_id: None,
            type_: UiEventType::WindowClose,
            payload: UiEventPayload::None,
        }
    }

    pub fn window_show() -> Self {
        Self::new(UiEventType::WindowShow, UiEventPayload::None)
    }

    pub fn window_hide() -> Self {
        Self::new(UiEventType::WindowHide, UiEventPayload::None)
    }

    pub fn window_occlusion_changed() -> Self {
        Self::new(UiEventType::WindowOcclusionChanged, UiEventPayload::None)
    }

    pub fn frame_opportunity(
        token: FrameRequestToken,
        frame_time: Instant,
        target_present_time: Option<Instant>,
    ) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::FrameOpportunity,
            payload: UiEventPayload::FrameOpportunity(FrameOpportunityData {
                token,
                frame_time,
                target_present_time,
            }),
        }
    }

    pub fn pointer_down(pos: Point, btn: MouseButton) -> Self {
        Self {
            window_id: None,
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
            window_id: None,
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
            window_id: None,
            type_: UiEventType::PointerMove,
            payload: UiEventPayload::PointerMove(PointerMoveEventData {
                pos,
                mods: KeyMod::NONE,
            }),
        }
    }

    pub fn wheel(pos: Point, delta_x: f32, delta_y: f32, mods: KeyMod) -> Self {
        Self {
            window_id: None,
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
            window_id: None,
            type_: UiEventType::KeyDown,
            payload: UiEventPayload::Key(KeyEventData { key, mods }),
        }
    }

    pub fn key_up(key: KeyCode, mods: KeyMod) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::KeyUp,
            payload: UiEventPayload::Key(KeyEventData { key, mods }),
        }
    }

    pub fn text_input(text: impl Into<String>) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::TextInput,
            payload: UiEventPayload::TextInput(TextInputData { text: text.into() }),
        }
    }

    pub fn ime_composition_start() -> Self {
        Self {
            window_id: None,
            type_: UiEventType::ImeCompositionStart,
            payload: UiEventPayload::None,
        }
    }

    pub fn ime_composition_update(text: impl Into<String>) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::ImeCompositionUpdate,
            payload: UiEventPayload::ImeComposition(ImeCompositionData { text: text.into() }),
        }
    }

    pub fn ime_composition_end(text: impl Into<String>) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::ImeCompositionEnd,
            payload: UiEventPayload::ImeComposition(ImeCompositionData { text: text.into() }),
        }
    }

    pub fn copy() -> Self {
        Self {
            window_id: None,
            type_: UiEventType::Copy,
            payload: UiEventPayload::None,
        }
    }

    pub fn cut() -> Self {
        Self {
            window_id: None,
            type_: UiEventType::Cut,
            payload: UiEventPayload::None,
        }
    }

    pub fn paste(text: impl Into<String>) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::Paste,
            payload: UiEventPayload::Clipboard(ClipboardData { text: text.into() }),
        }
    }

    pub fn theme_changed(is_dark: bool) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::ThemeChanged,
            payload: UiEventPayload::ThemeChanged(ThemeChangeData { is_dark }),
        }
    }

    pub fn locale_changed(locale: impl Into<String>) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::LocaleChanged,
            payload: UiEventPayload::LocaleChanged(LocaleChangeData {
                locale: locale.into(),
            }),
        }
    }

    pub fn timer(timer_id: u32) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::Timer,
            payload: UiEventPayload::Timer(TimerEventData { timer_id }),
        }
    }

    pub fn resize(width: i32, height: i32) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::WindowResize,
            payload: UiEventPayload::Resize(ResizeData { width, height }),
        }
    }

    pub fn file_drop(files: Vec<String>, position: Point) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::FileDrop,
            payload: UiEventPayload::FileDrop(FileDropData { files, position }),
        }
    }
}
