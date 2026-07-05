//! 组件系统 — Widget 框架、布局、主题、内置组件与呈现桥接。

pub mod animation;
pub mod core;
pub mod foundation;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod scene;
pub mod theme;
pub mod widgets;

pub use core::widget as widget;
pub use core::widget::{
    BoxedWidget, EventResult, IntoWidgetNode, WidgetCore, WidgetEvent, WidgetEventKind,
    WidgetId, WidgetNode, WidgetTree,
};
pub use crate::platform::{KeyCode, KeyMod, MouseButton};
pub use core::{children, context};
pub use crate::api::widget::WidgetComponent;
pub use foundation::{
    clipboard, config, focus_trap, locale, state, style, virtual_scroll,
};
