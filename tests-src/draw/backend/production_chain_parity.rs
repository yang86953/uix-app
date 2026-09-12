//! 真实 GPU parity 专用的 Drawing 生产入口组合。
//!
//! 本模块只在显式共享 parity feature 下把 UI 绘制闭包送入生产
//! `PaintContext`/`Canvas2D` lowering；它不知道或选择任何原生图形 API。

use crate::core::{Errc, Error, PresentDamage, Result};
use crate::draw::FontHandle;
use crate::draw::backend::gpu::{GpuGlyphBlit, NativeGpuCanvas2D, NativeRasterCaps};
use crate::draw::backend::rhi_renderer::{RhiRenderer, RhiSampledQuad};
use crate::draw::geometry::spatial::Orientation;
use crate::draw::painting::{PaintContext, PaintSurfaceConfig};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::platform::presentation::rhi::{
    GraphicsContextRhi, GraphicsDevice, GraphicsSurface, LoadAction, RhiColor, RhiExtent,
    RhiViewport, SurfaceToken, TextureDesc, TextureFormat, TextureHandle,
};

// 让真实 UI 绘制入口形成 Canvas 队列、共享 FramePlan 与一次 Device submit。
pub(crate) fn execute_ui_production_chain(
    device: &mut dyn GraphicsDevice,
    extent: RhiExtent,
    paint_ui: impl FnOnce(&mut PaintContext<'_>),
) -> Result<TextureHandle> {
    if !extent.is_valid() {
        return Err(Error::new(
            Errc::InvalidArgument,
            "UI production-chain parity requires a valid RHI extent",
        ));
    }
    let capabilities = device.device_capabilities();
    let native_caps = NativeRasterCaps::from_device_capabilities(capabilities);
    if !capabilities.has_gpu_baseline() || !native_caps.has_gpu_only_baseline() {
        return Err(Error::new(
            Errc::InvalidArgument,
            "UI production-chain parity requires the production GPU-only baseline",
        ));
    }
    let target = device.create_texture(TextureDesc::new(extent, TextureFormat::Rgba8Unorm))?;
    let mut canvas =
        NativeGpuCanvas2D::new_gpu_only(extent.width as i32, extent.height as i32, native_caps);
    let font_service = FontService::new();
    let image_service = ImageService::new();
    {
        let mut draw_context = PaintContext::new(
            &mut canvas,
            FontHandle::new(0),
            &font_service,
            &image_service,
            PaintSurfaceConfig {
                dpi: 96.0,
                device_pixel_ratio: 1.0,
                orientation: Orientation::YDown,
                surface_w: extent.width as i32,
                surface_h: extent.height as i32,
            },
        );
        paint_ui(&mut draw_context);
    }
    let mut renderer = RhiRenderer::default();
    let submitted = canvas.submit_rhi_solid(
        &mut renderer,
        device,
        extent,
        LoadAction::Clear(RhiColor::transparent()),
        target,
    )?;
    if !submitted {
        return Err(Error::new(
            Errc::InvalidState,
            "UI production-chain scene did not enter the shared FramePlan path",
        ));
    }
    Ok(target)
}

// 把真实 UI 绘制结果通过共享 sampled FramePlan 呈现到当前原生 Surface。
pub(crate) fn execute_ui_production_surface_chain(
    context: &mut dyn GraphicsContextRhi,
    paint_ui: impl FnOnce(&mut PaintContext<'_>),
) -> Result<SurfaceToken> {
    // 普通 parity 帧不安装观察者，仍复用同一个最终 Surface 事务实现。
    execute_ui_production_surface_chain_with_present_hook(context, paint_ui, &mut |_| {})
}

// 显式 parity 可在唯一 submit 后、present 前观察最终 Surface，不改变生产 FramePlan。
pub(crate) fn execute_ui_production_surface_chain_with_present_hook(
    context: &mut dyn GraphicsContextRhi,
    paint_ui: impl FnOnce(&mut PaintContext<'_>),
    before_present: &mut dyn FnMut(&mut dyn GraphicsSurface),
) -> Result<SurfaceToken> {
    // Surface token 是本次 acquire/render/present 事务的唯一代际与尺寸事实。
    let token = context.surface_ref().token();
    // 先复用真实 UI → Canvas2D → offscreen FramePlan 路径生成 retained 纹理。
    let target = execute_ui_production_chain(context.device(), token.extent, paint_ui)?;
    // 最终合成只使用共享物理 viewport，不读取任何 OS 或图形 API 值。
    let viewport = RhiViewport {
        width: token.extent.width as f32,
        height: token.extent.height as f32,
    };
    // 复用 Drawing GPU Module 的统一四角顺序构造全幅 sampled quad。
    let quad = RhiSampledQuad {
        x: 0.0,
        y: 0.0,
        w: viewport.width,
        h: viewport.height,
        corners: GpuGlyphBlit::axis_aligned_corners(0.0, 0.0, viewport.width, viewport.height),
        rgba: [1.0; 4],
        additive: false,
        texture: target,
        u0: 0.0,
        v0: 0.0,
        u1: 1.0,
        v1: 1.0,
        surface_corner_radius: 0.0,
        surface_shadow_fill: [0.0; 4],
        surface_shadow_fill_range: 0.0,
        scissor: None,
    };
    // 统一 Renderer 负责唯一 acquire、Device submit 与最终 Surface present。
    let present = RhiRenderer::default().execute_sampled_quads_with_present_hook(
        context,
        PresentDamage::Full,
        viewport,
        LoadAction::Clear(RhiColor::transparent()),
        std::slice::from_ref(&quad),
        before_present,
    );
    // 成功或失败后都检查式释放本次临时 retained 纹理。
    let cleanup = context.device().destroy_texture(target);
    match (present, cleanup) {
        (Ok(()), Ok(())) => {
            // 已呈现的 SUBOPTIMAL 可以在同一次调用中发布恰好一个后续代际。
            let current = context.surface_ref().token();
            if current.generation != token.generation
                && current.generation != token.generation.saturating_add(1)
            {
                return Err(Error::new(
                    Errc::GraphicsSurfaceLost,
                    "UI production-chain present advanced more than one surface generation",
                ));
            }
            // 返回 present 后的权威 token，禁止把已重建的旧 token 报告为当前代际。
            Ok(current)
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Err(error), Err(cleanup_error)) => Err(cleanup_error.with_source(error)),
    }
}
