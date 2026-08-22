//! 真实 GPU parity 专用的 Drawing 生产入口组合。
//!
//! 本模块只在显式共享 parity feature 下把 UI 绘制闭包送入生产
//! `PaintContext`/`Canvas2D` lowering；它不知道或选择任何原生图形 API。

use crate::core::{Errc, Error, Result};
use crate::draw::FontHandle;
use crate::draw::backend::gpu::{NativeGpuCanvas2D, NativeRasterCaps};
use crate::draw::backend::rhi_renderer::RhiRenderer;
use crate::draw::geometry::spatial::Orientation;
use crate::draw::painting::{PaintContext, PaintSurfaceConfig};
use crate::draw::resources::font::font_service::FontService;
use crate::draw::resources::image::ImageService;
use crate::platform::presentation::rhi::{
    GraphicsDevice, LoadAction, RhiColor, RhiExtent, TextureDesc, TextureFormat, TextureHandle,
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
