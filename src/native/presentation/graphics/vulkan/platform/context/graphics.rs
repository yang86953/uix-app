//! IGraphicsContext 实现。

use super::*;

impl IGraphicsContext for VulkanContext {
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsApi::Vulkan, self.device_pixel_ratio())
    }

    fn graphics_backend(&self) -> GraphicsApi {
        GraphicsApi::Vulkan
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        let result = self.resize_active(width, height);
        device.observe(result)
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn width(&self) -> i32 {
        self.width
    }

    fn height(&self) -> i32 {
        self.height
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.width as f32 / self.logical_width.max(1) as f32
    }

    fn present_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
        _damage: PresentDamage,
    ) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let result = self
            .upload_pixels(pixels, width, height)
            .and_then(|()| self.present_uploaded_pixels());
        device.observe(result)
    }

    // 统一 present 入口只允许当前 Vulkan PixelUpload recipe 的载荷。
    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<()> {
        // 根据显式 payload 保持 CPU upload 与 swapchain recipe 正交。
        match frame {
            // PixelBuffer 复用已经检查 device health 的上传入口。
            PresentFrame::PixelBuffer {
                // 借用调用方提供的 premultiplied 像素。
                pixels,
                // 保留 payload 的物理宽度。
                width,
                // 保留 payload 的物理高度。
                height,
                // 保留最终 present damage。
                damage,
            } => self.present_pixels(pixels, *width, *height, damage.clone()),
            // 当前 recipe 不允许无像素载荷的 swapchain 提交。
            PresentFrame::Swapchain { .. } => Err(Error::new(
                // 使用参数错误标记 recipe 与 payload 不匹配。
                Errc::InvalidArgument,
                // 明确要求调用方使用 PixelBuffer，而不是寻找旁路。
                "VulkanContext: Swapchain payload is invalid for the PixelUpload recipe",
            )),
        }
    }
}

// 为 GFX-R5 保留 Vulkan PixelUpload recipe 的显式诊断回读。
impl VulkanContext {
    // 从 staging hydration 后的 CPU shadow 读取指定区域。
    pub(crate) fn readback_pixels(
        // 借用当前 Vulkan owner context。
        &mut self,
        // 接收回读区域左上角横坐标。
        x: i32,
        // 接收回读区域左上角纵坐标。
        y: i32,
        // 接收回读区域宽度。
        width: i32,
        // 接收回读区域高度。
        height: i32,
    ) -> Result<Vec<u32>> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        let result = (|| {
            self.hydrate_cpu_shadow_from_staging()?;
            let expected = (self.width as usize).saturating_mul(self.height as usize);
            if self.cpu_shadow.len() != expected {
                return Err(Error::new(
                    Errc::InvalidState,
                    "VulkanContext: no uploaded frame to read back (cpu_shadow empty)",
                ));
            }
            crop_cpu_shadow(
                &self.cpu_shadow,
                self.width,
                self.height,
                x,
                y,
                width,
                height,
            )
        })();
        device.observe(result)
    }
}
