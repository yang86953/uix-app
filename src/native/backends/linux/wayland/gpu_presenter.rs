// ============================================================================
// platform/linux/wayland/gpu_presenter.rs — GPU 呈现器
//
// GpuPresenter 实现 IPresenter，不持有像素缓冲。GPU 引擎直接渲染到
// EGL 帧缓冲，present() 时调用 eglSwapBuffers 实现零拷贝呈现。
// ============================================================================

use crate::core::Error;
use crate::native::present::{IGraphicsContext, IPresenter, PresentDamage, PresentFrame};
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
        // 把外部 presenter 的 damage 封装为唯一的 context present payload。
        let frame = PresentFrame::Swapchain { damage };
        // 禁止 Wayland presenter 绕过统一 present 边界直接交换 EGL surface。
        self.gpu_ctx.present(&frame)
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // Wayland GPU presenter 只通过 thin RHI surface 重建 EGL drawable。
        self.gpu_ctx.resize_rhi_surface(width, height)
    }
}
