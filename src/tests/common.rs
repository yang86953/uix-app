//! Shared imports for crate tests under `src/tests`.
//! Prefer this over reintroducing production `#[path]` mounts.

pub(crate) use crate::core::log::Level;
pub(crate) use crate::core::{
    ComponentId, Constraints, DamageRegion, DirtyRegion, EdgeInsets, Errc, Error, Point, Rect,
    Size, WindowId,
};
pub(crate) use crate::draw::api::{PaintContext, ThemeSnapshot};
pub(crate) use crate::draw::command::{FrameEncoder, FrameRect};
pub(crate) use crate::draw::geometry::path::{FillRule, LineCap, LineJoin};
pub(crate) use crate::draw::geometry::spatial::{Mat4, Vec3, AABB3D};
pub(crate) use crate::draw::geometry::types::ImageHandle;
pub(crate) use crate::draw::renderer::NodeId;
pub(crate) use crate::draw::renderer::RenderTarget;
pub(crate) use crate::draw::renderer::{RecoveryAction, RenderOutcome};
pub(crate) use crate::draw::scene::PicturePolicy;
pub(crate) use crate::draw::StrokeOptions;
pub(crate) use crate::draw::{Color, FontHandle, FontService, ImageService, Renderer};
pub(crate) use crate::native::traits::input::{
    ControlSize, KeyCode, KeyMod, MouseButton, ScrollDirection,
};
pub(crate) use crate::native::traits::present::{
    GraphicsBackend, GraphicsContextCaps, IGraphicsContext, NativeRasterCaps, PresentCoherency,
    PresentDamage, PresentFrame, PresentMode, RasterMode, SoftFallbackTile,
};
pub(crate) use crate::native::traits::system::StatusLevel;
pub(crate) use crate::ui::core::widget::WidgetCore;
pub(crate) use crate::ui::layout::{AlignItems, FlexDirection};
pub(crate) use crate::ui::style::Style;
pub(crate) use crate::ui::theme::{DesignTokens, DynTokens, Theme};
pub(crate) use crate::ui::traits::{
    EventHandler, WidgetAnimation, WidgetCapabilities, WidgetComponent, WidgetLayout, WidgetRender,
};
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

/// Serializes real D3D12 WARP fixtures across libtest worker threads.
///
/// The Windows WARP runtime can access-violate inside `d3d10warp.dll` when
/// independent devices are created and torn down concurrently. UIX owns native
/// graphics on one UI thread, so only true WARP fixtures share this guard; the
/// rest of the suite remains parallel.
#[cfg(feature = "d3d12")]
pub(crate) fn d3d12_warp_test_guard() -> std::sync::MutexGuard<'static, ()> {
    static D3D12_WARP_TEST_LOCK: Mutex<()> = Mutex::new(());
    D3D12_WARP_TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}
