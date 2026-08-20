// EGL 的共享 Surface、recipe 与 OpenGL RHI host 生命周期实现。

// 复用父平台模块中的 EGL owner 及其私有原生辅助函数。
use super::EglContext;
// 引入共享 Surface 生命周期契约。
use crate::native::present::{GraphicsContextLifecycle, PresentDamage};
// 引入共享 OpenGL RHI host 生命周期契约。
use crate::native::presentation::graphics::opengl::rhi_host::OpenGlRhiHost;
// 引入共享 Surface resize 事务。
use crate::native::present::rhi::RhiSurfaceResizeTransaction;
// 引入共享 OpenGL raster pipeline 类型。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
// 引入统一错误与结果类型。
use crate::native::{Error, Result};

// 为 EGL owner 实现共享 graphics context 生命周期。
impl GraphicsContextLifecycle for EglContext {
    // 返回 EGL drawable 的完整 live surface 快照。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // resize 事务开始前必须能读取 windowing 刚发布的新 DPR。
        match self.metrics.snapshot() {
            // 健康快照直接报告同代 drawable 与 DPR。
            Ok(snapshot) => crate::native::present::PresentSurface::identity(
                // 报告目标物理 drawable 宽度。
                snapshot.drawable_width,
                // 报告目标物理 drawable 高度。
                snapshot.drawable_height,
                // Wayland core scale 是当前设备像素比。
                snapshot.scale as f32,
                // native 与 metrics 两侧代次取最大值拒绝旧帧。
                self.surface_generation.max(snapshot.revision),
            ),
            // trait 无错误通道时回退到最后一次成功应用的 EGL 状态。
            Err(_) => crate::native::present::PresentSurface::identity(
                // 最后成功物理宽度。
                self.width,
                // 最后成功物理高度。
                self.height,
                // 最后成功应用的 DPR。
                self.device_pixel_ratio,
                // 最后成功 native surface 代次。
                self.surface_generation,
            ),
        }
    }

    // 将 checked shutdown 交给 EGL owner 的唯一清理事务。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 复用同一关闭状态机，避免生命周期事实分叉。
        self.shutdown_result()
    }
}

// 把 EGL thin RHI 与逻辑 surface resize 收敛到同一 recipe owner。
impl crate::native::present::GpuRecipeContext for EglContext {
    // 借用 EGL owner 已实现的组合 thin RHI。
    fn rhi_context(
        // 借用当前 EGL owner。
        &mut self,
    ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi, Error> {
        // 在借出 Device/Surface 组合能力前先执行 owner 生命周期门禁。
        self.ensure_rhi_active()?;
        // 同一实例完整实现 GraphicsDevice 与 GraphicsSurface。
        Ok(self)
    }

    // 复用统一 DPR、范围检查与 GraphicsSurface::resize 调用。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 在可变借用前取得当前完整 surface 快照。
        let present_surface = GraphicsContextLifecycle::present_surface(self);
        // 直接借用当前原子 recipe owner，不经过分裂兼容视图。
        crate::native::present::resize_native_rhi_surface(self, present_surface, width, height)
    }
}

// 将 EGL 原生生命周期接入共享 OpenGL RHI host。
impl OpenGlRhiHost for EglContext {
    // 检查 EGL owner 的统一 active 状态。
    fn rhi_ensure_active(&self) -> Result<()> {
        // 复用 EGL inherent helper，避免 shared host 猜测句柄状态。
        self.ensure_rhi_active()
    }

    // 借用可变 raster/RHI owner。
    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline {
        &mut self.pipeline
    }

    // 借用只读 raster/RHI owner。
    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline {
        &self.pipeline
    }

    // 切换到 EGL owner-thread context。
    fn rhi_make_current(&mut self) -> Result<(), Error> {
        // current 只作为 adapter 私有 RHI host 操作存在。
        self.make_current_result()
    }

    // 返回 EGL surface generation。
    fn rhi_generation(&self) -> u64 {
        self.surface_generation
    }

    // 消费已验证事务并进入 EGL 原生 surface resize helper。
    fn rhi_resize_surface(
        // 借用当前 EGL owner。
        &mut self,
        // 接收共享门禁冻结的目标 extent 与原生投影。
        resize: RhiSurfaceResizeTransaction,
    ) -> Result<(), Error> {
        // 读取事务中不可替换的请求 extent。
        let extent = resize.extent();
        // 物理 extent 相同但 logical extent 或 DPR 改变时仍须同步 pipeline。
        let snapshot = self.metrics.snapshot()?;
        // 只有四项 surface 事实全都一致才可跳过。
        if self.pipeline.rhi_surface_extent() == extent
            && self.logical_width == snapshot.logical_width
            && self.logical_height == snapshot.logical_height
            && self.device_pixel_ratio == snapshot.scale as f32
        {
            // 当前 EGL 与 pipeline 已满足完整请求。
            return Ok(());
        }
        // 进入 EGL resize helper 前恢复 owner-thread current context。
        self.rhi_make_current()?;
        // 读取共享事务已经证明安全的原生有符号尺寸。
        let (width, height) = resize.native_size_i32();
        // 使用唯一投影重建原生 surface。
        self.resize_surface_extent(width, height)
    }

    // 交换 EGL window surface。
    fn rhi_swap_buffers(&mut self, _damage: PresentDamage) -> Result<(), Error> {
        // 业务门禁已在 shared host present 中完成，cleanup 不复用该门禁。
        self.egl
            .swap_buffers(self.display, self.surface)
            .map_err(super::map_egl_swap_error)
    }
}
