//! 界面能力 — 组件框架、布局、主题、内置组件与声明式 View。

pub mod animation;
pub mod core;
pub mod foundation;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod theme;
pub mod traits;
pub mod view;
pub mod widgets;

pub use core::widget as widget;
pub use core::widget::{
    BoxedWidget, EventResult, IntoWidgetNode, WidgetCore, WidgetEvent, WidgetEventKind,
    WidgetId, WidgetNode, WidgetTree,
};
pub use crate::native::{KeyCode, KeyMod, MouseButton};
pub use core::{children, context};
pub use foundation::{
    clipboard, config, focus_trap, locale, state, style, virtual_scroll,
};
pub use animation::{Animation, Easing};
pub use children::WidgetChildren;
pub use clipboard::copy_to_clipboard;
pub use foundation::state::{Computed, Effect, State};
pub use foundation::style::{Style, StyleVariant};
pub use theme::{DesignTokens, ShadowToken, Theme};
pub use layout::{
    AlignItems, FlexDirection, GridTrack, JustifyContent, LayoutChild, LayoutEngine, FlexLayout,
    GridLayout,
};
pub use crate::draw::painting::RenderContext;
pub use traits::{
    Animatable, DebugRenderer, TextRenderer, TokenProvider, WidgetCapabilities,
    WidgetComponent, WidgetEventHandler, WidgetLayout, WidgetLifecycle, WidgetRender,
};
pub use widgets::*;
