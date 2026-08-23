// ============================================================================
// platform/windowing/event/types.rs — 平台无关事件类型
// ============================================================================

use crate::core::WindowId;
use crate::core::geometry::Point;
use crate::platform::windowing::{KeyCode, KeyMod, MouseButton};
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
    PointerDoubleClick,
}

impl UiEventType {
    /// 返回不含事件载荷的固定诊断名称，供脱敏日志与复现清单使用。
    pub(crate) const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::WindowClose => "window_close",
            Self::WindowResize => "window_resize",
            Self::WindowMinimize => "window_minimize",
            Self::WindowMaximize => "window_maximize",
            Self::WindowRestore => "window_restore",
            Self::WindowFocus => "window_focus",
            Self::WindowBlur => "window_blur",
            Self::FrameOpportunity => "frame_opportunity",
            Self::PointerDown => "pointer_down",
            Self::PointerUp => "pointer_up",
            Self::PointerMove => "pointer_move",
            Self::Wheel => "wheel",
            Self::KeyDown => "key_down",
            Self::KeyUp => "key_up",
            Self::Copy => "copy",
            Self::Cut => "cut",
            Self::Paste => "paste",
            Self::TextInput => "text_input",
            Self::ImeCompositionStart => "ime_composition_start",
            Self::ImeCompositionUpdate => "ime_composition_update",
            Self::ImeCompositionEnd => "ime_composition_end",
            Self::Timer => "timer",
            Self::FileDrop => "file_drop",
            Self::ThemeChanged => "theme_changed",
            Self::LocaleChanged => "locale_changed",
            Self::WindowShow => "window_show",
            Self::WindowHide => "window_hide",
            Self::WindowOcclusionChanged => "window_occlusion_changed",
            Self::PointerDoubleClick => "pointer_double_click",
        }
    }
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

// 指针激活身份只建立原生输入与同轮窗口动作的因果关联。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
// 名义可见性与 PlatformWindow trait 一致，私有字段仍阻止外部构造或解释。
pub struct PointerActivationId(u64);

// 为平台后端提供唯一的受控身份构造入口。
impl PointerActivationId {
    // 非 Linux 构建不编译 Wayland 签发者，只保留跨平台契约。
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    // 仅允许 platform windowing 在签发原生输入授权时创建身份。
    pub(crate) const fn new(raw: u64) -> Self {
        // 保存不可解释的单调编号。
        Self(raw)
        // 结束身份构造实现。
    }
    // 结束不透明身份的方法集合。
}

#[derive(Debug, Clone, PartialEq)]
pub struct PointerButtonEventData {
    pub pos: Point,
    pub btn: MouseButton,
    pub mods: KeyMod,
    // 激活身份只供 app 在处理当前原生事件时转交平台窗口。
    pub(crate) activation: Option<PointerActivationId>,
}

impl Default for PointerButtonEventData {
    fn default() -> Self {
        Self {
            pos: Point::default(),
            btn: MouseButton::None,
            mods: KeyMod::NONE,
            // 默认事件不携带任何原生输入授权。
            activation: None,
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

    // 非 Linux 构建不产生 Wayland 激活身份，只保留事件接线契约。
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    // 将平台签发的激活身份附着到对应的指针按键事件。
    pub(crate) fn with_pointer_activation(mut self, activation: PointerActivationId) -> Self {
        // 仅指针按键负载可以承载该次激活身份。
        if let UiEventPayload::PointerButton(data) = &mut self.payload {
            // 保存身份而不向 UI 映射层解释其内容。
            data.activation = Some(activation);
            // 结束负载类型检查。
        }
        // 返回带单次执行上下文的原生事件。
        self
        // 结束激活身份附着方法。
    }

    // 读取当前原生指针事件携带的激活身份。
    pub(crate) const fn pointer_activation(&self) -> Option<PointerActivationId> {
        // 非指针按键事件永远没有可转交的拖动授权。
        match &self.payload {
            // 返回后端签发的不可解释身份。
            UiEventPayload::PointerButton(data) => data.activation,
            // 其他负载保持无授权语义。
            _ => None,
            // 结束负载投影。
        }
        // 结束激活身份读取方法。
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
                // 普通构造器不伪造原生输入激活身份。
                activation: None,
            }),
        }
    }

    pub fn pointer_double_click(pos: Point, btn: MouseButton) -> Self {
        Self {
            window_id: None,
            type_: UiEventType::PointerDoubleClick,
            payload: UiEventPayload::PointerButton(PointerButtonEventData {
                pos,
                btn,
                mods: KeyMod::NONE,
                // 双击事件不复用任一原生按下授权。
                activation: None,
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
                // 抬起事件只负责撤销后端授权，不携带拖动身份。
                activation: None,
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
