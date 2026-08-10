//! API-neutral non-GL GPU-native raster backend.
//!
//! Hot Canvas2D paths (`fill_rect` / `fill_circle` / `fill_sector` /
//! `stroke_rect` / `stroke_circle`, axis-aligned `draw_line`, solid `blit_glyph`,
//! linear/radial gradients, fill-rule-aware `fill_path`,
//! cap/join-aware `stroke_path`, box/ambient shadow, and scaled image blit)
//! 通过固定 probe 验证的 retained RHI 执行绘制，并由 [`NativeRasterCaps`] 陈述事实能力。
//! Axis-aligned transforms are folded into device geometry; general affine
//! sharp fills and strict-GPU ellipses use solid meshes. Every unsupported
//! operation deterministically soft-rasterizes into a CPU buffer and alpha-blits
//! at present (or typed-fails in GPU-only mode).

use std::time::Duration;

const SOFT_FALLBACK_IDLE_PRESENT_GRACE: u8 = 2;
pub(crate) const SOFT_FALLBACK_IDLE_TIME_GRACE: Duration = Duration::from_millis(250);
pub(crate) mod backend;
// 隐藏 graphics backend 私有的 renderer 能力投影实现。
mod capabilities;
pub(crate) mod canvas;
pub(crate) mod canvas2d;
pub(crate) mod geometry;
pub(crate) mod helpers;
pub(crate) mod pending;
pub(crate) mod queue;
pub(crate) mod submit;
pub(crate) mod tile;

pub use backend::{GpuBackend, NativeGpuDrawSurface};
// 向 graphics backend 内部消费者统一导出私有能力投影。
pub(crate) use capabilities::NativeRasterCaps;
pub(crate) use canvas::NativeGpuCanvas2D;
pub(crate) use pending::StateSnapshot;
