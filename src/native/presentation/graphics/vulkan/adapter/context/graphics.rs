//! Vulkan PixelUpload context 生命周期实现。

use super::*;

impl GraphicsContextLifecycle for VulkanContext {
    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    // 返回 Vulkan PixelUpload swapchain 的完整 drawable 快照。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // FullOnly recipe 不消费 generation，保留此前默认零代际语义。
        crate::native::present::PresentSurface::identity(
            // 记录当前 swapchain 物理宽度。
            self.width,
            // 记录当前 swapchain 物理高度。
            self.height,
            // 从同一状态计算逻辑到物理的 DPR。
            self.width as f32 / self.logical_width.max(1) as f32,
            // 保持 PixelUpload damage tracker 的既有 generation。
            0,
        )
    }
}

// 为 Vulkan CPU PixelUpload recipe 实现专用 surface 生命周期。
impl PixelUploadSurface for VulkanContext {
    // 上传 CPU retained pixels 并提交当前 Vulkan PixelUpload swapchain。
    fn present_pixels(
        // 借用 Vulkan PixelUpload owner。
        &mut self,
        // 接收 premultiplied BGRA 像素。
        pixels: &[u32],
        // 接收物理像素宽度。
        width: i32,
        // 接收物理像素高度。
        height: i32,
        // Vulkan 当前执行完整上传，保留 damage 作为 recipe 语义输入。
        _damage: PresentDamage,
    ) -> Result<()> {
        // 获取当前共享 device lease。
        let device = self.active_device()?;
        // present 前拒绝已经丢失的 device。
        device.ensure_healthy()?;
        // 非正 extent 没有可提交像素，保持既有空操作语义。
        if width <= 0 || height <= 0 {
            // 空像素提交成功且不触碰 swapchain。
            return Ok(());
        }
        // 先上传 staging pixels，再提交已经取得的 swapchain image。
        let result = self
            // 把 CPU pixels 写入当前 Vulkan upload buffer。
            .upload_pixels(pixels, width, height)
            // 只有上传成功才进入唯一 WSI present。
            .and_then(|()| self.present_uploaded_pixels());
        // 让共享 device 观察并分类提交结果。
        device.observe(result)
    }

    // 重建 Vulkan swapchain 并保持 device-lost typed 映射。
    fn resize_pixel_upload_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 获取当前共享 device lease。
        let device = self.active_device()?;
        // resize 前拒绝已经丢失的 device。
        device.ensure_healthy()?;
        // 执行 Vulkan surface 的实际 resize。
        let result = self.resize_active(width, height);
        // 让共享 device 观察并分类 resize 结果。
        device.observe(result)
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
