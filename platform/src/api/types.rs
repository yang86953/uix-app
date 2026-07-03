//! # uix-platform 数据契约
//!
//! 本模块定义 platform 层的共享数据结构（值类型）。
//! 这些类型是跨层的数据契约，不作为行为抽象。

// ── 错误类型 ──
pub use crate::error::{
    collect_errors, collect_values, make_error, to_std_error_code, try_invoke, Errc, Error,
    ErrorSeverity, Result, ResultErrorExt, ResultExt,
};

// ── 呈现损伤 ──
pub use crate::present_damage::PresentDamage;

// ── 几何类型 ──
pub use crate::geometry::{EdgeInsets, Point, Rect, Size};

// ── 事件类型 ──
pub use crate::event::{
    KeyEventData, MouseButtonEventData, MouseMoveEventData, MouseWheelData, ResizeData, UiEvent,
    UiEventPayload, UiEventType,
};

// ── 事件总线 ──
pub use crate::event_bus::{EventBus, EventHandler};

// ── 按键与输入 ──
pub use crate::types::{
    ConsoleColor, ControlSize, CursorType, DisplayInfo, KeyCode, KeyMod, MemoryInfo, MouseButton,
    OsInfo, ScrollDirection, SpecialDir, StatusLevel, TerminalCapabilities,
};
