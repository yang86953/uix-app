// 复用父模块中的 WGL context 定义及其私有原生辅助方法。
use super::WglContext;

// 引入共享 OpenGL RHI host 合约。
use crate::native::presentation::graphics::opengl::rhi_host::OpenGlRhiHost;
// 引入 WGL owner 持有的 raster pipeline 类型。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
// 引入共享 Surface 生命周期、resize 与重建事务。
use crate::platform::presentation::rhi::{
    RhiExtent, RhiSurfaceLifecycle, RhiSurfaceRecreateReason, RhiSurfaceRecreateTransaction,
    RhiSurfaceResizeTransaction,
};
// 引入共享 present damage 类型。
use crate::native::present::{GraphicsContextLifecycle, PresentDamage};
// 引入窗口 drawable 尺寸换算辅助函数。
use crate::native::presentation::graphics::platform::windows::drawable_size_from_hdc;
// 引入项目统一错误类型。
use crate::native::{Error, Result};

// 将 WGL 原生生命周期接入共享 OpenGL RHI host。
impl OpenGlRhiHost for WglContext {
    // 检查 WGL owner 的统一 active 状态。
    fn rhi_ensure_active(&self) -> Result<()> {
        // 复用 WGL inherent helper，避免 shared host 猜测句柄状态。
        self.ensure_rhi_active()
    }

    // 借用可变 raster/RHI owner。
    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline {
        // 返回 WGL context 持有的唯一 pipeline。
        &mut self.pipeline
    }

    // 借用只读 raster/RHI owner。
    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline {
        // 返回 WGL context 持有的唯一 pipeline。
        &self.pipeline
    }

    // 切换到 WGL owner-thread context。
    fn rhi_make_current(&mut self) -> Result<(), Error> {
        // 委托给 WGL context 的原生 current 操作。
        self.make_current_result()
    }

    // 借用唯一共享 Surface 生命周期。
    fn rhi_surface_lifecycle(&self) -> &RhiSurfaceLifecycle {
        &self.surface_lifecycle
    }

    // 可变借用同一个共享 Surface 生命周期。
    fn rhi_surface_lifecycle_mut(&mut self) -> &mut RhiSurfaceLifecycle {
        &mut self.surface_lifecycle
    }

    // 判断 WGL drawable 是否已经满足共享 resize 请求。
    fn rhi_surface_matches(&self, resize: RhiSurfaceResizeTransaction) -> Result<bool> {
        // WGL 没有独立逻辑 revision，物理 drawable 一致即可跳过。
        Ok(self.pipeline.rhi_surface_extent() == resize.extent())
    }

    // 机械消费共享事务并完成 WGL 原生 Surface 操作。
    fn rhi_recreate_surface(
        &mut self,
        recreate: RhiSurfaceRecreateTransaction,
    ) -> Result<RhiExtent, Error> {
        // 请求 extent 已由共享生命周期验证并封闭保存。
        let extent = recreate.requested();
        if recreate.reason() == RhiSurfaceRecreateReason::Initialize {
            return Err(crate::native::Error::new(
                crate::native::Errc::InvalidState,
                "WglContext: initialize transaction reached active host",
            ));
        }
        // acquire/present 丢失必须先重新取得真正的 Win32 HDC Surface。
        if matches!(
            recreate.reason(),
            RhiSurfaceRecreateReason::AcquisitionRejected
                | RhiSurfaceRecreateReason::PresentationRejected
                | RhiSurfaceRecreateReason::PresentedNeedsRecreate
        ) {
            self.recreate_device_context()?;
        }
        // 读取当前窗口的 device pixel ratio。
        // 从单一 surface 快照读取 DPR，避免分离元数据发生撕裂。
        let dpr = self.present_surface().device_pixel_ratio.max(0.0001);
        // 将物理宽度换算为至少一个像素的逻辑宽度。
        let logical_width = (extent.width as f32 / dpr).round().max(1.0) as i32;
        // 将物理高度换算为至少一个像素的逻辑高度。
        let logical_height = (extent.height as f32 / dpr).round().max(1.0) as i32;
        // 根据 HDC 和逻辑尺寸计算实际 drawable 尺寸。
        let drawable = drawable_size_from_hdc(self.hwnd, self.hdc, logical_width, logical_height);
        // 交给原生 helper 只更新 drawable 与 pipeline，不复制 generation。
        self.resize_surface_drawable(drawable)?;
        // Adapter 只报告原生操作实际产生的正 extent。
        Ok(RhiExtent::new(self.width as u32, self.height as u32))
    }

    // 交换 WGL double-buffer surface。
    fn rhi_swap_buffers(&mut self, _damage: PresentDamage) -> Result<(), Error> {
        // 将交换失败保留为 GraphicsSurfaceLost 类型错误。
        self.swap_buffers_result()
    }
}
