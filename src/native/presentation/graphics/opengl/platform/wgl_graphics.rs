// 复用父模块中的 WGL context 及 owner-thread 原生辅助方法。
use super::WglContext;

// 引入 OpenGL ES 图形上下文与呈现契约。
use crate::native::present::{IGraphicsContext, NativeRasterCaps, PresentCoherency, PresentFrame};
// 引入项目统一错误和结果类型。
use crate::native::{Errc, Error, Result};
// 为 WGL context 实现共享的 IGraphicsContext forwarding 合约。
impl IGraphicsContext for WglContext {
    // 返回 OpenGL ES swapchain 能力和当前 device pixel ratio。
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        // WGL 目前只承诺完整交换，不伪造 partial present preservation。
        crate::native::present::GraphicsContextCaps::gpu_native_swapchain(
            crate::native::present::GraphicsApi::OpenGlEs,
            PresentCoherency::FullOnly,
        )
    }

    // 暴露同一 owner-thread context 上的 OpenGL ES 薄 RHI 组合视图。
    fn rhi_context(&mut self) -> Option<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // WGL adapter 已经实现共享 GraphicsDevice/GraphicsSurface。
        Some(self)
    }

    // 返回固定 RHI probe 已兑现的事实能力。
    fn native_raster_caps(&self) -> NativeRasterCaps {
        // WGL 只公布 retained surface 与 Additive pipeline 事实。
        NativeRasterCaps::retained_rhi_with_additive()
    }

    // 返回当前图形 backend 标识。
    fn graphics_backend(&self) -> crate::native::present::GraphicsApi {
        // WGL adapter 使用 OpenGL ES backend 标签。
        crate::native::present::GraphicsApi::OpenGlEs
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

    // 统一 present 入口只接受 native swapchain frame。
    fn present(&mut self, frame: &PresentFrame) -> Result<(), Error> {
        // 按 present payload 区分 native swapchain 与 CPU pixel buffer。
        match frame {
            // native swapchain 进入 owner-thread current 和交换。
            PresentFrame::Swapchain { .. } => {
                // 先确保 WGL context 为 current。
                self.make_current_result()?;
                // 统一 presenter 入口不能绕过 lower surface-lost 故障。
                #[cfg(feature = "test-harness")]
                if self.pipeline.rhi_take_surface_lost_for_test() {
                    // 保留同一 typed surface-lost 分类。
                    tracing::warn!("OpenGL RHI test surface lost");
                    return Err(Error::new(
                        Errc::GraphicsSurfaceLost,
                        "OpenGL RHI test surface lost before present",
                    ));
                }
                // 交换 WGL double-buffer surface。
                self.swap_buffers_result()
            }
            // WGL 不支持 CPU pixel buffer present。
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                Errc::NotImplemented,
                "WglContext: CPU pixel present is unsupported",
            )),
        }
    }

    // 关闭 WGL context 并释放 owner-thread GPU 资源。
    fn try_shutdown(&mut self) -> Result<(), Error> {
        // 委托给包含 pipeline release 的 shutdown helper。
        self.shutdown_result()
    }
}
