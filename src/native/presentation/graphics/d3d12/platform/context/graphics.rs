use super::*;

impl IGraphicsContext for D3d12Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsApi::D3d12,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn initialize(&mut self, _native_window: *mut c_void, _width: i32, _height: i32) -> Result<()> {
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.resize_result(width, height)
    }

    fn make_current(&mut self) -> Result<()> {
        self.begin_commands()
    }

    fn swap_buffers(&mut self, _damage: PresentDamage) -> Result<()> {
        self.present_result()
    }

    fn present(&mut self, frame: &PresentFrame<'_>) -> Result<()> {
        match frame {
            PresentFrame::Swapchain { .. } => self.present_result(),
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                Errc::InvalidArgument,
                "D3d12Context: PixelBuffer present is not supported",
            )),
        }
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }

    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        self.read_pixels_result(x, y, width, height)
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
}

impl Drop for D3d12Context {
    fn drop(&mut self) {
        if let Err(error) = self.shutdown_result() {
            tracing::error!(
                "D3d12Context: undrained Drop retained GPU COM objects: {}",
                error.short_what()
            );
            self.retain_gpu_objects_after_undrained_drop();
        }
    }
}
