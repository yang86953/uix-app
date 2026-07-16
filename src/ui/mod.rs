//! 界面能力 — 组件框架、布局、主题、内置组件与声明式 View。

pub(crate) mod accessibility_override;
pub mod animation;
pub mod app_state;
#[cfg(feature = "test-harness")]
pub mod automation;
pub mod component_handle;
pub(crate) mod component_patch;
pub mod component_snapshot;
pub(crate) mod core;
pub mod event;
pub mod focus_handle;
pub mod foundation;
pub mod layout;
pub mod macros;
pub mod managers;
pub mod overlay;
pub mod placement;
pub(crate) mod render_handler;
pub(crate) mod semantic_action;
pub(crate) mod semantic_snapshot;
pub(crate) mod system_event_handler;
pub mod theme;
pub mod traits;
pub mod view;
pub mod widgets;
pub mod window_chrome;

pub use crate::core::ComponentId;
pub use crate::draw::painting::PaintContext;
pub use crate::native::traits::input::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub use crate::native::traits::system::StatusLevel;
pub use animation::{Animated, Animation, AnimationConfig, Easing};
pub use app_state::AppState;
pub use clipboard::{copy_to_clipboard, read_text_from_clipboard};
pub use component_handle::ComponentHandle;
pub use component_snapshot::{
    AccessibilityRole, AccessibilitySnapshot, AccessibilityState, AriaAttribute,
    ComponentConfigSnapshot, SelectionSnapshot, SnapshotCollapsePanel, SnapshotField,
    SnapshotFields, SnapshotSource, SnapshotTableColumn, SnapshotTableColumnGroup,
    SnapshotTransferItem, SnapshotTreeNode, SnapshotValue,
};
pub use core::children::WidgetChildren;
pub(crate) use core::widget::WidgetTree;
pub use core::widget::{EventResult, IntoWidgetNode};
pub(crate) use core::{children, widget};

// Public component traits and exported macros mention these opaque bridge
// types. Keep them nameable without making the runtime module hierarchy an
// application-facing API.
#[doc(hidden)]
pub mod __private {
    pub use super::core::widget::{WidgetNode, WidgetTree};
}
pub use event::{
    ClickEvent, HandlerId, HandlerOptions, HandlerRegistration, HandlerTable, SemanticEvent,
    SemanticKind, SemanticPayload, SystemEvent, SystemEventKind,
};
pub use focus_handle::{FocusHandle, FocusHandleError};
pub use foundation::config::{
    render_empty_for, use_config, with_config, ComponentConfig, ComponentOverrides,
    ComponentTokenOverrides, Config, ConfigProvider, EmptyContext, EmptyRenderer,
};
pub use foundation::locale::{en_us, use_locale, with_locale, zh_cn, Locale, LocaleProvider};
pub use foundation::state::{Computed, Effect, State, StateSlotId};
pub use foundation::style::{
    ColorValue, PaletteColor, Style, StyleSet, StyleState, TypographyToken,
};
pub use foundation::virtual_scroll::{VirtualScroll, VirtualScrollBuilder};
pub use foundation::{clipboard, config, focus_trap, locale, state, style, virtual_scroll};
pub use layout::{
    AlignItems, BoxModel, FlexDirection, FlexLayout, GridLayout, GridTrack, JustifyContent,
    LayoutChild, LayoutEngine, LayoutOutput,
};
pub use managers::{
    DragManager, FocusManager, InteractionManager, StateManager, TextManager, WidgetManagers,
};
pub use overlay::{OverlayEntry, OverlayId, OverlayKind, OverlayStack};
pub use placement::Placement;
pub use theme::{
    generate_color_scale, ColorScale, DataVisualizationPalette, DesignTokens, DynTokens,
    FunctionalColorRole, NeutralColorScale, NeutralRole, PrimaryHue, ShadowToken, Theme,
    ThemePrimitives, TokenPatch, DATA_VISUALIZATION_PALETTE, NEUTRAL_PALETTE,
};
pub use traits::{
    Animatable, DebugRenderer, EventHandler, TextRenderer, TokenProvider, WidgetAnimation,
    WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetLifecycle, WidgetRender,
};
pub use widgets::*;
pub use window_chrome::{window_control, window_control_named, window_drag_region, WindowControl};

#[cfg(feature = "test-harness")]
pub mod test_harness {
    pub use super::automation::{
        AutomationAction, AutomationActionKind, AutomationError, AutomationErrorCode,
        AutomationNode, AutomationSelection, AutomationSnapshot, AutomationTarget, TestApp,
        AUTOMATION_DIR_ENV, AUTOMATION_SCHEMA,
    };
    pub use super::core::widget::{WidgetCore, WidgetTree};
    pub use super::view::adapter::ViewAdapter;
}
