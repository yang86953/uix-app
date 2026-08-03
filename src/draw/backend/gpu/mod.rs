//! API-neutral non-GL GPU-native raster backend.
//!
//! Hot Canvas2D paths (`fill_rect` / `fill_circle` / `fill_sector` /
//! `stroke_rect` / `stroke_circle`, axis-aligned `draw_line`, solid `blit_glyph`,
//! linear/radial gradients, fill-rule-aware `fill_path`,
//! cap/join-aware `stroke_path`, box/ambient shadow, and scaled image blit)
//! draw via [`IGraphicsContext`] operations advertised by [`NativeRasterCaps`].
//! Axis-aligned transforms are folded into device geometry; general affine
//! sharp fills and strict-GPU ellipses use solid meshes. Every unsupported
//! operation deterministically soft-rasterizes into a CPU buffer and alpha-blits
//! at present (or typed-fails in GPU-only mode).

use std::any::Any;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::core::{DamageRegion, Errc, Error, Point, PresentDamageTracker, Rect};
use crate::draw::backend::contract::{
    BackendCapabilities, BackendKind, DrawSurface, RenderBackend,
};
use crate::draw::geometry::color::Color;
use crate::draw::geometry::path::{FillRule, Path, PathBuilder};
use crate::draw::geometry::stroker::StrokeOptions;
use crate::draw::geometry::tessellator;
use crate::draw::geometry::types::{BlendMode, GradientDirection, ImageHandle, Radius, Transform};
use crate::draw::painting::{
    EncodedFrameExecution, EncodedPictureExecution, FrameCommand, FrameEncoder, FrameEncoderError,
    FrameGlyphBlit, FrameRasterOp, FrameRect, FrameStrokeRect, ReferenceFrame,
};
use crate::draw::raster::pixel_surface::PixelSurface;
use crate::draw::raster::rasterizer::core::align_rounded_rect;
use crate::draw::raster::shared_rasterizer::SharedRasterizer;
use crate::draw::Canvas2D;
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect, IGraphicsContext, NativeRasterCaps,
    OffscreenTargetId, PresentFrame, PresentMode, PresentTestResult, RasterMode, SoftFallbackTile,
};

const SOFT_FALLBACK_IDLE_PRESENT_GRACE: u8 = 2;
pub(crate) const SOFT_FALLBACK_IDLE_TIME_GRACE: Duration = Duration::from_millis(250);
pub(crate) mod backend;
pub(crate) mod canvas;
pub(crate) mod canvas2d;
pub(crate) mod geometry;
pub(crate) mod helpers;
pub(crate) mod pending;
pub(crate) mod queue;
pub(crate) mod submit;
pub(crate) mod tile;

pub use backend::{GpuBackend, NativeGpuDrawSurface};
pub(crate) use canvas::NativeGpuCanvas2D;
pub(crate) use pending::{PendingNativeOp, StateSnapshot};
pub(crate) use tile::pack_visible_soft_fallback_tile;
