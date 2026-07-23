//! Widget 组件契约 — trait 定义。

pub mod animation;
pub mod layout;
pub mod theme;
pub mod widget;

pub use animation::Animatable;
pub use layout::LayoutEngine;
pub use theme::{IBoxShadowTokens, ISpacingTokens, ITypographyTokens, ThemeTokens, TokenProvider};
pub use widget::{
    EventHandler, IntoWidgetNode, WidgetAnimation, WidgetCapabilities, WidgetComponent,
    WidgetLayout, WidgetLifecycle, WidgetRender, WidgetTextInput,
};
