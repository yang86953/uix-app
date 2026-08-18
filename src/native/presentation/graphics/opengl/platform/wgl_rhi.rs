// 复用父模块中的 WGL context 定义及其私有原生辅助方法。
use super::WglContext;

// 引入共享 OpenGL RHI host 合约。
use crate::native::presentation::graphics::opengl::rhi_host::OpenGlRhiHost;
// 引入 WGL owner 持有的 raster pipeline 类型。
use crate::native::presentation::graphics::opengl::raster::OpenGlRasterPipeline;
// 引入已经通过共享门禁的 surface resize 事务。
use crate::native::present::rhi::RhiSurfaceResizeTransaction;
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

    // 返回 WGL surface generation。
    fn rhi_generation(&self) -> u64 {
        // 返回 surface 重建时递增的 generation。
        self.surface_generation
    }

    // 将已验证物理 extent 转回窗口逻辑尺寸并进入原生 drawable resize helper。
    fn rhi_resize_surface(
        // 借用当前 WGL owner。
        &mut self,
        // 接收共享门禁冻结的目标 extent。
        resize: RhiSurfaceResizeTransaction,
    ) -> Result<(), Error> {
        // 读取事务中不可替换的请求 extent。
        let extent = resize.extent();
        // 相同物理尺寸无需重复重建 drawable。
        if self.pipeline.rhi_surface_extent() == extent {
            // 把已满足的 resize 请求视为成功。
            return Ok(());
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
        // 交给原生 drawable resize helper 完成 surface generation 更新。
        self.resize_surface_drawable(drawable)
    }

    // 交换 WGL double-buffer surface。
    fn rhi_swap_buffers(&mut self, _damage: PresentDamage) -> Result<(), Error> {
        // 将交换失败保留为 GraphicsSurfaceLost 类型错误。
        self.swap_buffers_result()
    }
}
