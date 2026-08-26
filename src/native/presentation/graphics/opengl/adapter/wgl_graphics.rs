// 复用父模块中的 WGL context 及 owner-thread 原生辅助方法。
use super::WglContext;

// 引入 OpenGL ES 图形 context 的共享生命周期契约。
use crate::platform::presentation::GraphicsContextLifecycle;
// 引入项目统一错误和结果类型。
use crate::native::{Error, Result};
// 为 WGL context 实现共享生命周期合约。
impl GraphicsContextLifecycle for WglContext {
    // 返回 WGL drawable 的完整 live surface 快照。
    fn present_surface(&self) -> crate::platform::presentation::PresentSurface {
        // generation 只读取共享生命周期，不再由 WGL 私有字段维护。
        let generation = self.surface_lifecycle.token().generation;
        // 逻辑宽度无效时保留既有安全比例。
        let device_pixel_ratio = if self.logical_width <= 0 {
            // 避免除零并保留可验证的 identity 映射。
            1.0
        } else {
            // 从同一次状态读取计算当前 drawable 比例。
            self.width as f32 / self.logical_width as f32
        };
        // 同时返回 extent、DPR 与真实 surface generation。
        crate::platform::presentation::PresentSurface::identity(
            // 记录当前物理 drawable 宽度。
            self.width,
            // 记录当前物理 drawable 高度。
            self.height,
            // 记录上方计算的稳定 DPR。
            device_pixel_ratio,
            // 共享重建事务发布的 generation 拒绝迟到帧。
            generation,
        )
    }

    // 关闭 WGL context 并释放 owner-thread GPU 资源。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 委托给包含 pipeline release 的 shutdown helper。
        self.shutdown_result()
    }
}

// 把 WGL thin RHI 与逻辑 surface resize 收敛到同一 recipe owner。
impl crate::platform::presentation::GpuRecipeContext for WglContext {
    // 借用 WGL owner 已实现的组合 thin RHI。
    fn rhi_context(
        // 借用当前 WGL owner。
        &mut self,
    ) -> Result<&mut dyn crate::platform::presentation::rhi::GraphicsContextRhi, Error> {
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
        crate::platform::presentation::resize_native_rhi_surface(
            self,
            present_surface,
            width,
            height,
        )
    }
}
