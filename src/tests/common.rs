//! Shared imports for crate tests under `src/tests`.
//! Prefer this over reintroducing production `#[path]` mounts.

pub(crate) use crate::core::{
    ComponentId, Constraints, DamageRegion, DirtyRegion, EdgeInsets, Errc, Error, Point, Rect,
    Size, WindowId,
};
pub(crate) use crate::draw::engine::{RecoveryAction, RenderOutcome};
pub(crate) use crate::draw::traits::GraphicsEngine;
pub(crate) use crate::draw::painting::{PaintContext, ThemeSnapshot};
pub(crate) use crate::draw::pipeline::{FrameEncoder, FrameRect, NodeId};
pub(crate) use crate::draw::primitives::path::{FillRule, LineCap, LineJoin};
pub(crate) use crate::draw::primitives::types::ImageHandle;
pub(crate) use crate::draw::spatial::{AABB3D, Mat4, Vec3};
pub(crate) use crate::draw::compositor::PicturePolicy;
pub(crate) use crate::draw::{
    Color, FontHandle, FontService, ImageService, NullEngine, SoftwareEngine,
};
pub(crate) use crate::native::traits::input::{
    ControlSize, CursorType, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub(crate) use crate::core::log::Level;
pub(crate) use crate::draw::StrokeOptions;
pub(crate) use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, NativeRasterCaps, PresentDamage,
    PresentFrame, PresentMode, RasterMode, SoftFallbackTile,
};
pub(crate) use crate::native::traits::system::StatusLevel;
pub(crate) use crate::ui::layout::{AlignItems, FlexDirection, JustifyContent};
pub(crate) use crate::ui::style::Style;
pub(crate) use crate::ui::theme::{DesignTokens, DynTokens, Theme};
pub(crate) use crate::ui::traits::{
    EventHandler, WidgetAnimation, WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
pub(crate) use crate::ui::core::widget::WidgetCore;
pub(crate) use crate::ui::{
    AppState, EventResult, OverlayKind, SemanticEvent, SemanticKind, SnapshotFields, SystemEvent,
    WidgetTree,
};
pub(crate) use std::cell::{Cell, RefCell};
pub(crate) use std::collections::{HashMap, HashSet, VecDeque};
pub(crate) use std::rc::Rc;
pub(crate) use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
pub(crate) use std::sync::{Arc, Mutex};
pub(crate) use std::time::{Duration, Instant};
