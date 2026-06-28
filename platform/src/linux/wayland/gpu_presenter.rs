// ============================================================================
// platform/linux/wayland/gpu_presenter.rs — GPU 呈现器
//
// GpuPresenter 实现 IPresenter，不持有像素缓冲。GPU 引擎直接渲染到
// EGL 帧缓冲，present() 时调用 eglSwapBuffers 实现零拷贝呈现。
// ============================================================================

use crate::Error;
use crate::{IGraphicsContext, IPresenter};

pub struct GpuPresenter {
    gpu_ctx: Box<dyn IGraphicsContext>,
}

impl GpuPresenter {
    pub fn new(gpu_ctx: Box<dyn IGraphicsContext>) -> Self {
        Self { gpu_ctx }
    }
}

impl IPresenter for GpuPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        self.gpu_ctx.swap_buffers();
        Ok(())
    }

    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        self.gpu_ctx.resize(width, height);
        Ok(())
    }
}
