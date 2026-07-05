//! Widget 组件契约 — trait 定义。

pub mod animation;
pub mod layout;
pub mod render;
pub mod theme;
pub mod widget;

pub use animation::Animatable;
pub use layout::LayoutEngine;
pub use render::{DebugRenderer, TextRenderer};
pub use theme::TokenProvider;
pub use widget::{
    IntoWidgetNode, WidgetCapabilities, WidgetComponent, WidgetEventHandler, WidgetLayout,
    WidgetLifecycle, WidgetRender,
};
