use super::*;

impl IGraphicsContext for D3d12Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(
            GraphicsApi::D3d12,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<()> {
        self.resize_result(width, height)
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
