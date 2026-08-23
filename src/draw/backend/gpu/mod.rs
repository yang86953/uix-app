//! API-neutral non-GL GPU-native raster backend.
//!
//! Hot Canvas2D paths (`fill_rect` / `fill_circle` / `fill_sector` /
//! `stroke_rect` / `stroke_circle`, analytic `draw_line`, solid `blit_glyph`,
//! linear/radial gradients, fill-rule-aware `fill_path`,
//! cap/join-aware `stroke_path`, box/ambient shadow, and scaled image blit)
//! 通过固定 probe 验证的 retained RHI 执行绘制，并由 `NativeRasterCaps` 陈述事实能力。
//! Axis-aligned transforms are folded into device geometry; general affine
//! sharp fills and strict-GPU ellipses use solid meshes. Every unsupported
//! operation deterministically soft-rasterizes into a CPU buffer and alpha-blits
//! at present (or typed-fails in GPU-only mode).

use std::time::Duration;

const SOFT_FALLBACK_IDLE_PRESENT_GRACE: u8 = 2;
pub(crate) const SOFT_FALLBACK_IDLE_TIME_GRACE: Duration = Duration::from_millis(250);
// Drawing GPU Module 私有拥有固定 pipeline 的 FramePlan 启动探针。
// 保留 `gpu::backend` 逻辑路径，实行与呈现实现物理归入 execution。
#[path = "execution/mod.rs"]
pub(crate) mod backend;
mod device_probe;
// 隐藏 graphics backend 私有的 renderer 能力投影实现。
pub(crate) mod canvas;
pub(crate) mod canvas2d;
mod capabilities;
pub(crate) mod geometry;
pub(crate) mod helpers;
pub(crate) mod pending;
// 隐藏 graphics backend 私有的 Canvas2D GPU 原语传输对象。
mod primitives;
pub(crate) mod queue;
pub(crate) mod submit;
pub(crate) mod tile;

pub use backend::{GpuBackend, NativeGpuDrawSurface};
// 向 graphics backend 内部消费者统一导出私有能力投影。
pub(crate) use canvas::NativeGpuCanvas2D;
pub(crate) use capabilities::NativeRasterCaps;
pub(crate) use pending::StateSnapshot;
// 向 graphics backend 内部消费者统一导出 renderer 原语。
pub(crate) use primitives::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLineSegment, GpuLinearGradientRect,
    GpuRadialGradient, GpuSector, GpuSolidMesh, GpuSolidRect, GpuStrokeRect,
};

// 将 GPU 启动探针的动态资源与 FramePlan 生命周期测试归属 Drawing GPU Module。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/draw/backend/gpu/device_probe_tests.rs"]
mod device_probe_tests;
