//! 界面能力 — 组件框架、布局、主题、内置组件与声明式 View。

pub mod animation;
pub mod core;
pub mod event;
pub mod foundation;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod theme;
pub mod traits;
pub mod view;
pub mod widgets;

pub use crate::draw::painting::PaintContext;
pub use crate::native::{ControlSize, KeyCode, KeyMod, MouseButton, StatusLevel};
pub use animation::{Animation, Easing};
pub use children::WidgetChildren;
pub use clipboard::copy_to_clipboard;
pub use core::widget;
pub use core::widget::{
    BoxedWidget, EventResult, IntoWidgetNode, WidgetCore, WidgetId, WidgetNode, WidgetTree,
};
pub use core::{children, context};
pub use event::{
    ClickEvent, HandlerId, HandlerOptions, HandlerRegistration, HandlerTable, SemanticEvent,
    SemanticKind, SemanticPayload, SystemEvent, SystemEventKind,
};
pub use foundation::state::{Computed, Effect, State};
pub use foundation::style::{Style, StyleSet, StyleState};
pub use foundation::{clipboard, config, focus_trap, locale, state, style, virtual_scroll};
pub use layout::{
    AlignItems, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent, LayoutChild,
    LayoutEngine,
};
pub use theme::{DesignTokens, ShadowToken, Theme};
pub use traits::{
    Animatable, DebugRenderer, EventHandler, TextRenderer, TokenProvider, WidgetCapabilities,
    WidgetComponent, WidgetLayout, WidgetLifecycle, WidgetRender,
};
pub use widgets::*;
