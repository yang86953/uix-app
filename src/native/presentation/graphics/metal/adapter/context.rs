//! Metal identity PixelUpload context — CAMetalLayer CPU upload path.
//!
//! Native GPU raster via Metal remains planned. This context declares
//! [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`] and does not own a Metal
//! device, command queue, or raster pipeline.

use std::ffi::c_void;

use crate::core::{Errc, Error, Result};
use crate::native::backends::macos::platform;
use crate::platform::presentation::{
    GraphicsContextLifecycle, PixelUploadSurface, PresentDamage, validate_pixel_buffer,
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

impl GraphicsContextLifecycle for MetalPixelUploadContext {
    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    // 返回 CAMetalLayer PixelUpload 的完整 drawable 快照。
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        // Metal PixelUpload 当前不声明同尺寸重建代际，保留零 generation。
        crate::platform::presentation::PresentSurface::identity(
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
}

// 为 Metal CPU PixelUpload recipe 实现专用 surface 生命周期。
impl PixelUploadSurface for MetalPixelUploadContext {
    // 把 CPU retained pixels 上传到 CAMetalLayer 并完成最终提交。
    fn present_pixels(
        // 借用当前 Metal PixelUpload owner。
        &mut self,
        // 接收 premultiplied BGRA 像素。
        pixels: &[u32],
        // 接收物理像素宽度。
        width: i32,
        // 接收物理像素高度。
        height: i32,
        // 接收最终提交 damage。
        damage: PresentDamage,
    ) -> Result<()> {
        // checked shutdown 后禁止继续访问 AppKit-owned layer。
        if !self.initialized {
            // 返回稳定参数错误，保持既有调用方分类。
            return Err(Error::new(
                // shutdown 后提交属于无效调用状态。
                Errc::InvalidArgument,
                // false 只可能来自 checked shutdown，不再代表等待第二阶段初始化。
                "MetalPixelUploadContext: present after shutdown",
            ));
        }
        // 提交前验证像素长度与物理 extent 一致。
        validate_pixel_buffer(pixels, width, height)?;
        // SAFETY: layer pointer comes from AppKit-owned CAMetalLayer on the UI thread.
        unsafe {
            // 把专用 PixelUpload payload 交给平台 CAMetalLayer helper。
            platform::present_layer_pixels(self.layer, pixels, width, height, damage)?;
        }
        // 只有平台 helper 成功后才报告提交完成。
        Ok(())
    }

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
