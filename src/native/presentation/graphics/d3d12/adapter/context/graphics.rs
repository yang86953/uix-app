use super::*;

impl GraphicsContextLifecycle for D3d12Context {
    // 返回 D3D12 测试期 context 的完整 drawable 元数据快照。
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

// 为尚未激活的 D3D12 registry row 固化 GPU recipe 类型形状。
impl crate::platform::presentation::GpuRecipeContext for D3d12Context {
    // D3D12 内部纵向切片完成后仍保持组合入口未激活的明确结果。
    fn rhi_context(
        // 借用当前 D3D12 owner。
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi> {
        // checked shutdown 后必须先拒绝，不得把未实现门禁伪装成存活能力。
        self.ensure_healthy()?;
        // Planned row 不得把内部实现伪造成 UI 到 Drawing 的生产链贯通。
        Err(Error::new(
            // 保持为实现缺口而非设备故障。
            Errc::NotImplemented,
            // 保留可诊断的激活门禁说明。
            "D3D12 thin RHI is not implemented",
        ))
    }

    // 复用 D3D12 已接入共享 Surface 权威的 swapchain resize 事务。
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
