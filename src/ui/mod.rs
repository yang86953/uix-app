//! 界面能力 — 组件框架、布局、主题、内置组件与声明式 View。

pub mod animation;
pub mod app_state;
pub mod component_handle;
pub mod component_snapshot;
pub mod core;
pub mod event;
pub mod foundation;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod overlay;
pub mod theme;
pub mod traits;
pub mod view;
pub mod widgets;

pub use crate::core::ComponentId;
pub use crate::draw::painting::PaintContext;
pub use crate::native::traits::input::{ControlSize, KeyCode, KeyMod, MouseButton};
pub use crate::native::traits::system::StatusLevel;
pub use animation::{Animation, Easing};
pub use app_state::AppState;
pub use children::WidgetChildren;
pub use clipboard::copy_to_clipboard;
pub use component_handle::ComponentHandle;
pub use component_snapshot::{
    ComponentConfigSnapshot, SnapshotCollapsePanel, SnapshotField, SnapshotFields, SnapshotSource,
    SnapshotTableColumn, SnapshotTransferItem, SnapshotTreeNode, SnapshotValue,
};
pub use core::widget;
pub use core::widget::{
    BoxedWidget, EventResult, IntoWidgetNode, WidgetCore, WidgetId, WidgetNode, WidgetTree,
};
pub use core::{children, context};
pub use event::{
    ClickEvent, HandlerId, HandlerOptions, HandlerRegistration, HandlerTable, SemanticEvent,
    SemanticKind, SemanticPayload, SystemEvent, SystemEventKind,
};
pub use foundation::state::{Computed, Effect, State, StateSlotId};
pub use foundation::style::{
    ColorValue, PaletteColor, Style, StyleSet, StyleState, TypographyToken,
};
pub use foundation::{clipboard, config, focus_trap, locale, state, style, virtual_scroll};
pub use layout::{
    AlignItems, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent, LayoutChild,
    LayoutEngine,
};
pub use managers::{
    DragManager, FocusManager, InteractionManager, StateManager, StyleManager, TextManager,
    WidgetManagers,
};
pub use overlay::{OverlayEntry, OverlayId, OverlayKind, OverlayStack};
pub use theme::{DesignTokens, NeutralRole, ShadowToken, Theme};
pub use traits::{
    Animatable, DebugRenderer, EventHandler, TextRenderer, TokenProvider, WidgetAnimation,
    WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetLifecycle, WidgetRender,
};
pub use widgets::*;
