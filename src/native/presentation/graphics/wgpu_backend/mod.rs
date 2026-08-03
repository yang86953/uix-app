//! Shared `wgpu` graphics context. All native GPU APIs execute the same UIX
//! renderer; backend selection changes only wgpu's adapter implementation.

pub(crate) mod blur;
pub(crate) mod draw_stream;
pub(crate) mod executor;
pub(crate) mod glyph_batch;
pub(crate) mod glyph_cover;
#[path = "platform/surface.rs"]
mod surface;

use std::ffi::c_void;
use std::sync::Arc;

use crate::core::{Errc, Error, Rect, Result};
use crate::diagnostics::{PendingFailureQueue, PendingFailureSource};
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect, GraphicsBackend, GraphicsContextCaps,
    IGraphicsContext, NativeRasterCaps, OffscreenTargetId, PresentCoherency, PresentDamage,
    PresentTestResult,
};

use blur::SeparableBlur;
use executor::{ActiveTarget, WgpuExecutor};

/// Prefer opaque composition so uncleared / partial-alpha pixels do not show the
/// desktop through a normal HWND swapchain. Premultiplied remains the fallback.
pub(crate) fn choose_surface_alpha_mode(
    modes: &[wgpu::CompositeAlphaMode],
) -> Option<wgpu::CompositeAlphaMode> {
    modes
        .iter()
        .copied()
        .find(|mode| *mode == wgpu::CompositeAlphaMode::Opaque)
        .or_else(|| {
            modes
                .iter()
                .copied()
                .find(|mode| *mode == wgpu::CompositeAlphaMode::PreMultiplied)
        })
        .or_else(|| modes.first().copied())
}

/// Swapchain draws use logical viewport for NDC; offscreen slots already store
/// their logical extent.
pub(crate) fn logical_draw_viewport(
    bound_offscreen: Option<(i32, i32)>,
    logical_width: i32,
    logical_height: i32,
) -> (f32, f32) {
    if let Some((width, height)) = bound_offscreen {
        return (width.max(1) as f32, height.max(1) as f32);
    }
    (logical_width.max(1) as f32, logical_height.max(1) as f32)
}

/// 在 downlevel 基线上抬高 2D 纹理上限，以覆盖桌面最大化 / 高分屏 swapchain。
///
/// `Limits::downlevel_defaults()` 的 `max_texture_dimension_2d` 仅为 2048；在
/// 1440p+ 显示器上最大化窗口会让 `Surface::configure` 失败，随后
/// `get_current_texture` 以 fatal panic 退出进程。
pub(crate) fn device_limits_for_adapter(adapter_limits: &wgpu::Limits) -> wgpu::Limits {
    let mut limits = wgpu::Limits::downlevel_defaults();
    limits.max_texture_dimension_2d = adapter_limits
        .max_texture_dimension_2d
        .max(limits.max_texture_dimension_2d);
    limits
}

/// 配置前校验 surface / retained 纹理尺寸，避免非法 configure 把 swapchain 留在未配置态。
pub(crate) fn ensure_surface_extent(
    width: u32,
    height: u32,
    max_texture_dimension_2d: u32,
) -> Result<(u32, u32)> {
    let width = width.max(1);
    let height = height.max(1);
    if width > max_texture_dimension_2d || height > max_texture_dimension_2d {
        return Err(Error::new(
            Errc::GraphicsOutOfMemory,
            format!(
                "wgpu surface {width}x{height} exceeds device max_texture_dimension_2d {max_texture_dimension_2d}"
            ),
        ));
    }
    Ok((width, height))
}

/// Keeps the legacy void destroy adapter on the callback-to-owner boundary.
///
/// The checked path still returns the original error to its owner. This
/// adapter has no return channel, so it must preserve the typed cause in the
/// context source instead of logging a lossy summary or invoking recovery.
fn enqueue_offscreen_destroy_failure(pending_failures: &PendingFailureSource, error: Error) {
    let _ = pending_failures.enqueue(error);
}

pub(crate) fn create_vulkan(
    surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(
        surface,
        width,
        height,
        GraphicsBackend::Vulkan,
        pending_failures,
    )
    .map(|context| Box::new(context) as _)
}

#[allow(dead_code)] // compiled on every target so platform cfg stays in platform/**
pub(crate) fn create_d3d12(
    surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(
        surface,
        width,
        height,
        GraphicsBackend::D3d12,
        pending_failures,
    )
    .map(|context| Box::new(context) as _)
}

#[allow(dead_code)] // compiled on every target so platform cfg stays in platform/**
pub(crate) fn create_opengl(
    surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(
        surface,
        width,
        height,
        GraphicsBackend::OpenGlEs,
        pending_failures,
    )
    .map(|context| Box::new(context) as _)
}

#[allow(dead_code)] // compiled on every target so platform cfg stays in platform/**
pub(crate) fn create_metal(
    surface: *mut c_void,
    width: i32,
    height: i32,
    pending_failures: PendingFailureQueue,
) -> Result<Box<dyn IGraphicsContext>, Error> {
    WgpuContext::new(
        surface,
        width,
        height,
        GraphicsBackend::Metal,
        pending_failures,
    )
    .map(|context| Box::new(context) as _)
}

struct OffscreenSlot {
    /// 保持纹理存活；采样与 RT / 模糊共用同一 allocation。
    #[allow(dead_code)]
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    width: i32,
    height: i32,
}

pub struct WgpuContext {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    _adapter: wgpu::Adapter,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    renderer: WgpuExecutor,
    backend: GraphicsBackend,
    native_surface: *mut c_void,
    logical_width: i32,
    logical_height: i32,
    width: i32,
    height: i32,
    max_texture_dimension_2d: u32,
    pending_failures: crate::diagnostics::PendingFailureSource,
    shutdown: bool,
    offscreens: Vec<Option<OffscreenSlot>>,
    free_offscreen_ids: Vec<u32>,
    next_offscreen_id: u32,
    bound_offscreen: Option<u32>,
    separable_blur: SeparableBlur,
}

mod context;
