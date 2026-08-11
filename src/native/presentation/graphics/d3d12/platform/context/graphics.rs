use super::*;

impl GraphicsContextLifecycle for D3d12Context {
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

// 为尚未激活的 D3D12 registry row 固化 GPU recipe 类型形状。
impl crate::native::present::GpuRecipeContext for D3d12Context {
    // D3D12 在 thin RHI 完成前保持明确的未实现探测结果。
    fn rhi_context(
        // 借用当前 D3D12 owner。
        &mut self,
    ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // Planned row 不得伪造已经完成的 thin RHI。
        Err(Error::new(
            // 保持为实现缺口而非设备故障。
            Errc::NotImplemented,
            // 保留可诊断的激活门禁说明。
            "D3D12 thin RHI is not implemented",
        ))
    }

    // 复用 D3D12 已有的 swapchain resize 事务。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<()> {
        // 在未来激活前保持单一 native resize 路径。
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
