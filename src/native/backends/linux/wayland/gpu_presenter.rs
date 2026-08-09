// ============================================================================
// platform/linux/wayland/gpu_presenter.rs — GPU 呈现器
//
// GpuPresenter 实现 IPresenter，不持有像素缓冲。GPU 引擎直接渲染到
// EGL 帧缓冲，present() 时调用 eglSwapBuffers 实现零拷贝呈现。
// ============================================================================

use crate::core::{Errc, Error};
use crate::native::present::{IGraphicsContext, IPresenter, PresentDamage};
pub(crate) struct GpuPresenter {
    gpu_ctx: Box<dyn IGraphicsContext>,
}

impl GpuPresenter {
    pub(crate) fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Self {
        Self { gpu_ctx }
    }
}

impl IPresenter for GpuPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        damage: PresentDamage,
    ) -> Result<(), Error> {
        // 构造后专用 swapchain 提交视图消失属于 native context 状态破坏。
        let Some(presentation) = self.gpu_ctx.swapchain_presentation() else {
            // 返回 typed 状态错误，禁止回退已移除的统一 present。
            return Err(Error::new(
                // 使用 InvalidState 进入既有恢复路径。
                Errc::InvalidState,
                // 明确指出 Wayland external presenter 契约缺失。
                "Wayland GPU presenter lost its dedicated swapchain presentation view",
            ));
        };
        // 只经 external presenter 专用视图提交已绘制的 EGL swapchain。
        presentation.present_swapchain(damage)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // Wayland GPU presenter 只通过 thin RHI surface 重建 EGL drawable。
        let Some(lifecycle) = self.gpu_ctx.rhi_surface_lifecycle() else {
            // 外部 GPU presenter recipe 丢失专用生命周期属于状态破坏。
            return Err(Error::new(
                // 使用 InvalidState 进入既有恢复路径。
                Errc::InvalidState,
                // 明确指出缺失的是 resize 专用视图。
                "Wayland GPU presenter lost its dedicated RHI surface lifecycle view",
            ));
        };
        // 让 EGL owner 经唯一 GraphicsSurface::resize 路径推进代际。
        lifecycle.resize_rhi_surface(width, height)
    }
}
