use super::*;

impl GraphicsContextLifecycle for D3d12Context {
    // 返回 D3D12 生产 context 的完整 drawable 元数据快照。
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        // 物理范围与 generation 必须来自同一个共享 token 快照。
        let token = self.surface_lifecycle.token();
        let width = token.extent.width as i32;
        let height = token.extent.height as i32;
        crate::platform::presentation::PresentSurface::identity(
            // 记录当前已发布的物理 backbuffer 宽度。
            width,
            // 记录当前已发布的物理 backbuffer 高度。
            height,
            // 从同一状态计算当前设备像素比。
            width as f32 / self.logical_width.max(1) as f32,
            // 共享重建事务发布的 generation 拒绝迟到 surface 快照。
            token.generation,
        )
    }

    fn try_shutdown(&mut self) -> Result<()> {
        self.shutdown_result()
    }
}

// 为 D3D12 生产 registry row 提供完整 GPU recipe 类型形状。
impl crate::platform::presentation::GpuRecipeContext for D3d12Context {
    // 借出同一 owner 已实现的 Device 与 Surface 组合视图。
    fn rhi_context(
        // 借用当前 D3D12 owner。
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
        // checked shutdown 或设备故障后必须在任何 RHI 操作前拒绝。
        self.ensure_healthy()?;
        // D3D12Context 同时实现 GraphicsDevice 与 GraphicsSurface，直接形成组合合同。
        Ok(self)
    }

    // 复用 D3D12 已接入共享 Surface 权威的 swapchain resize 事务。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 保持单一 native resize 路径与共享 Surface 生命周期。
        self.resize_result(width, height)
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
