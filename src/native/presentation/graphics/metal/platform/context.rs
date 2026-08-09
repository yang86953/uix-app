//! Metal identity PixelUpload context — CAMetalLayer CPU upload path.
//!
//! Native GPU raster via Metal remains planned. This context declares
//! [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`] and does not own a Metal
//! device, command queue, or raster pipeline.

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform;
use crate::native::present::{
    validate_pixel_buffer, GraphicsApi, GraphicsContextCaps, IGraphicsContext, PixelUploadSurface,
    PresentDamage, PresentFrame,
};

type LayerId = *mut c_void;

pub struct MetalPixelUploadContext {
    layer: LayerId,
    width: i32,
    height: i32,
    device_pixel_ratio: f32,
    initialized: bool,
}

impl MetalPixelUploadContext {
    pub(crate) fn new(native_surface: *mut c_void, width: i32, height: i32) -> Result<Self> {
        if native_surface.is_null() {
            return Err(Error::new(
                Errc::PlatformError,
                "MetalPixelUploadContext: native CAMetalLayer surface is null",
            ));
        }
        Ok(Self {
            layer: native_surface,
            width: width.max(1),
            height: height.max(1),
            device_pixel_ratio: 1.0,
            // 构造成功即表示 CAMetalLayer 与初始 drawable 尺寸已经就绪。
            initialized: true,
        })
    }

    fn shutdown_result(&mut self) -> Result<()> {
        self.initialized = false;
        Ok(())
    }
}

impl IGraphicsContext for MetalPixelUploadContext {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Metal)
    }

    // Metal PixelUpload recipe 显式暴露专用 surface resize。
    fn pixel_upload_surface(&mut self) -> Option<&mut dyn PixelUploadSurface> {
        // 返回当前 CAMetalLayer pixel-upload owner。
        Some(self)
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    // 返回 CAMetalLayer PixelUpload 的完整 drawable 快照。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // Metal PixelUpload 当前不声明同尺寸重建代际，保留零 generation。
        crate::native::present::PresentSurface::identity(
            // 记录当前 layer 上传宽度。
            self.width,
            // 记录当前 layer 上传高度。
            self.height,
            // 记录构造时确定的设备像素比。
            self.device_pixel_ratio,
            // 保持此前默认 PresentSurface 的 generation。
            0,
        )
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        damage: PresentDamage,
    ) -> Result<()> {
        if !self.initialized {
            return Err(Error::new(
                Errc::InvalidArgument,
                // false 只可能来自 checked shutdown，不再代表等待第二阶段初始化。
                "MetalPixelUploadContext: present after shutdown",
            ));
        }
        validate_pixel_buffer(pixels, width, height)?;
        // SAFETY: layer pointer comes from AppKit-owned CAMetalLayer on the UI thread.
        unsafe {
            platform::present_layer_pixels(self.layer, pixels, width, height, damage)?;
        }
        Ok(())
    }

    fn present(&mut self, frame: &PresentFrame) -> Result<()> {
        match frame {
            PresentFrame::PixelBuffer {
                pixels,
                width,
                height,
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
            PresentFrame::Swapchain { .. } => Err(Error::new(
                Errc::NotImplemented,
                "MetalPixelUploadContext: swapchain present requires native Metal raster (planned)",
            )),
        }
    }
}

// 为 Metal CPU PixelUpload recipe 实现专用 surface 生命周期。
impl PixelUploadSurface for MetalPixelUploadContext {
    // 保存归一化后的逻辑 PixelUpload surface 尺寸。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 将非正宽度归一化为最小可用值。
        self.width = width.max(1);
        // 将非正高度归一化为最小可用值。
        self.height = height.max(1);
        // Metal layer 的实际 drawable 更新由 present helper 负责。
        Ok(())
    }
}
