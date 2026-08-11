// 复用父模块中的 WGL context 及 owner-thread 原生辅助方法。
use super::WglContext;

// 引入 OpenGL ES 图形上下文与呈现一致性契约。
use crate::native::present::IGraphicsContext;
// 引入项目统一错误和结果类型。
use crate::native::{Error, Result};
// 为 WGL context 实现共享的 IGraphicsContext forwarding 合约。
impl IGraphicsContext for WglContext {
    // 暴露同一 owner-thread context 上不可拆分的 OpenGL ES GPU recipe 视图。
    fn gpu_recipe_context(&mut self) -> Option<&mut dyn crate::native::present::GpuRecipeContext> {
        // WGL 同时拥有 thin RHI 与唯一 GraphicsSurface::resize 路径。
        Some(self)
    }

    // 返回 WGL drawable 的完整 live surface 快照。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // 逻辑宽度无效时保留既有安全比例。
        let device_pixel_ratio = if self.logical_width <= 0 {
            // 避免除零并保留可验证的 identity 映射。
            1.0
        } else {
            // 从同一次状态读取计算当前 drawable 比例。
            self.width as f32 / self.logical_width as f32
        };
        // 同时返回 extent、DPR 与真实 surface generation。
        crate::native::present::PresentSurface::identity(
            // 记录当前物理 drawable 宽度。
            self.width,
            // 记录当前物理 drawable 高度。
            self.height,
            // 记录上方计算的稳定 DPR。
            device_pixel_ratio,
            // resize 重建时推进的 generation 拒绝迟到帧。
            self.surface_generation,
        )
    }

    // 关闭 WGL context 并释放 owner-thread GPU 资源。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 委托给包含 pipeline release 的 shutdown helper。
        self.shutdown_result()
    }
}

// 把 WGL thin RHI 与逻辑 surface resize 收敛到同一 recipe owner。
impl crate::native::present::GpuRecipeContext for WglContext {
    // 借用 WGL owner 已实现的组合 thin RHI。
    fn rhi_context(
        // 借用当前 WGL owner。
        &mut self,
    ) -> Result<&mut dyn crate::native::present::rhi::GraphicsContextRhi, Error> {
        // 同一实例完整实现 GraphicsDevice 与 GraphicsSurface。
        Ok(self)
    }

    // 复用统一 DPR、范围检查与 GraphicsSurface::resize 调用。
    fn resize_surface(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 在可变借用前取得当前完整 surface 快照。
        let present_surface = IGraphicsContext::present_surface(self);
        // 直接借用当前原子 recipe owner，不经过分裂兼容视图。
        crate::native::present::resize_native_rhi_surface(self, present_surface, width, height)
    }
}
