//! Common native application and UI mechanism contracts.
#[cfg(feature = "application")]
pub use crate::app::DiContainer;
#[cfg(feature = "application")]
pub use crate::app::{
    App, AppHandle, AppMode, DesktopLayerConfig, TimerHandle, WindowConfig, WindowSurfaceRole,
    map_ui_event,
};
#[cfg(feature = "core")]
pub use crate::core::{
    Constraints, EdgeInsets, Errc, Error, ErrorSeverity, Point, Rect, Size, WidgetId, WindowId,
};
#[cfg(feature = "data")]
pub use crate::data::SettingsService;
#[cfg(feature = "graphics")]
pub use crate::draw::{
    BlendMode, Color, FillRule, FontBundle, FontService, ImageService, Path, PathBuilder, Radius,
    Renderer, StrokeOptions, colors,
};
#[cfg(feature = "platform-contracts")]
pub use crate::platform::graphics::GraphicsBackend;
#[cfg(feature = "platform-contracts")]
pub use crate::platform::windowing::{DesktopAnchor, DesktopKeyboardInteractivity, DesktopLayer};
#[cfg(feature = "layout")]
pub use crate::ui::layout::{GridTrackMax, GridTrackMin};
#[cfg(feature = "test-harness")]
#[cfg(feature = "test-harness")]
pub use crate::ui::test_harness::TestApp;
#[cfg(feature = "ui")]
pub use crate::ui::theme::style::*;
#[cfg(feature = "ui")]
pub use crate::ui::*;
#[cfg(all(feature = "reactive", not(feature = "ui")))]
pub use crate::ui::{Computed, Effect, State};
#[cfg(feature = "ui")]
pub use crate::{
    impl_widget, keyframe, semantic_handler, t, t_fmt_arg, tree, uix, uix_items, uix_module, views,
    widget, with_cloned,
};
