//! IGraphicsContext 实现。

use super::*;

impl IGraphicsContext for VulkanContext {
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        GraphicsContextCaps::cpu_pixel_upload(GraphicsBackend::Vulkan, self.device_pixel_ratio())
    }

    fn graphics_backend(&self) -> GraphicsBackend {
        GraphicsBackend::Vulkan
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        let result = self.resize_active(width, height);
        device.observe(result)
    }

    fn make_current(&mut self) -> Result<()> {
        Ok(())
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        Err(Error::new(
            Errc::NotImplemented,
            "VulkanContext: swapchain present is not supported for the PixelUpload recipe; use PresentFrame::PixelBuffer",
        ))
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
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

    /// Stage a full replace pixel buffer into the upload heap without presenting.
    /// Also refreshes the CPU shadow used by [`Self::read_pixels`] so destination-
    /// dependent IR can round-trip through readback → apply → replace upload.
    fn upload_surface_pixels(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<()> {
        let device = self.active_device()?;
        device.ensure_healthy()?;
        if width <= 0 || height <= 0 {
            return Ok(());
        }
        let result = self.upload_pixels(pixels, width, height);
        device.observe(result)
    }
}

