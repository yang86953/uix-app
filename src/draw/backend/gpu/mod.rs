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

use std::time::Duration;


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
pub(crate) use pending::StateSnapshot;
