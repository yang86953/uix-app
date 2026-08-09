use super::*;

impl IGraphicsContext for D3d12Context {
    fn caps(&self) -> GraphicsContextCaps {
        GraphicsContextCaps::gpu_native_swapchain(GraphicsApi::D3d12, PresentCoherency::FullOnly)
    }

    // 返回 D3D12 测试期 context 的完整 drawable 元数据快照。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // D3D12 尚未承诺跨 resize generation，保留既有零代际语义。
        crate::native::present::PresentSurface::identity(
            // 记录当前物理 backbuffer 宽度。
            self.width,
            // 记录当前物理 backbuffer 高度。
            self.height,
            // 从同一状态计算当前设备像素比。
            self.width as f32 / self.logical_width.max(1) as f32,
            // 保持此前默认 PresentSurface 的 generation。
            0,
        )
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
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
