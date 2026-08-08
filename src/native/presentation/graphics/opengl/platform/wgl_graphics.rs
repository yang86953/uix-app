// 复用父模块中的 WGL context 及 owner-thread 原生辅助方法。
use super::WglContext;

// 引入窗口初始化使用的原生句柄类型。
use std::ffi::c_void;

// 引入 OpenGL ES legacy native queue 的所有绘制 DTO。
use crate::native::present::{
    GpuBoxShadow, GpuGlyphBlit, GpuImageBlit, GpuLinearGradientRect, GpuRadialGradient, GpuSector,
    GpuSolidMesh, GpuSolidRect, GpuStrokeRect, IGraphicsContext, NativeRasterCaps,
    OffscreenTargetId, PresentCoherency, PresentDamage, PresentFrame, SoftFallbackTile,
};
// 引入项目统一错误和结果类型。
use crate::native::{Errc, Error, Result};
// 引入 WGL drawable 尺寸换算辅助函数。
use crate::native::presentation::graphics::platform::windows::drawable_size_from_hdc;

// 为 WGL context 实现共享的 IGraphicsContext forwarding 合约。
impl IGraphicsContext for WglContext {
    // 返回 OpenGL ES swapchain 能力和当前 device pixel ratio。
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        // WGL 目前只承诺完整交换，不伪造 partial present preservation。
        crate::native::present::GraphicsContextCaps::gpu_native_swapchain(
            crate::native::present::GraphicsApi::OpenGlEs,
            PresentCoherency::FullOnly,
            self.device_pixel_ratio(),
        )
    }

    // 暴露同一 owner-thread context 上的 OpenGL ES 薄 RHI 组合视图。
    fn rhi_context(&mut self) -> Option<&mut dyn crate::native::present::rhi::GraphicsContextRhi> {
        // WGL adapter 已经实现共享 GraphicsDevice/GraphicsSurface。
        Some(self)
    }

    // 返回 legacy queue 与通用 RHI 共同兑现的能力表。
    fn native_raster_caps(&self) -> NativeRasterCaps {
        // OpenGL ES 的 GPU-only 录制子集由通用 RHI 或 legacy compatibility owner 执行。
        NativeRasterCaps::rhi_gpu_only_subset()
    }

    // 返回当前图形 backend 标识。
    fn graphics_backend(&self) -> crate::native::present::GraphicsApi {
        // WGL adapter 使用 OpenGL ES backend 标签。
        crate::native::present::GraphicsApi::OpenGlEs
    }

    // 兼容生命周期初始化由 WGL constructor 完成。
    fn initialize(
        &mut self,
        _native_window: *mut c_void,
        _width: i32,
        _height: i32,
    ) -> Result<(), Error> {
        // 构造阶段已经完成所有原生初始化。
        Ok(())
    }

    // 按逻辑窗口尺寸重建 WGL drawable。
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error> {
        // 将逻辑尺寸换算为真实 drawable 尺寸。
        let drawable = drawable_size_from_hdc(self.hwnd, self.hdc, width, height);
        // 委托给 owner-thread resize helper。
        self.resize_surface_drawable(drawable)
    }

    // 切换当前线程的 WGL context。
    fn make_current(&mut self) -> Result<(), Error> {
        // 保持 typed native error 语义。
        self.make_current_result()
    }

    // 交换 WGL surface 并消费 surface-lost 注入。
    fn swap_buffers(&mut self, damage: PresentDamage) -> Result<(), Error> {
        // 兼容 presenter 也必须消费共享 OpenGL lower surface-lost 注入。
        #[cfg(feature = "test-harness")]
        if self.pipeline.rhi_take_surface_lost_for_test() {
            // 保留与共享 RHI surface host 相同的故障 marker。
            tracing::warn!("OpenGL RHI test surface lost");
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "OpenGL RHI test surface lost before present",
            ));
        }
        // WGL 交换暂不使用 damage hint。
        let _ = damage;
        // 将 SwapBuffers 失败映射为 GraphicsSurfaceLost。
        self.swap_buffers_result()
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

    // 读取当前 WGL framebuffer 的 RGBA 像素。
    fn read_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>, Error> {
        // readback 前确保 context current。
        self.make_current_result()?;
        // 委托给 OpenGL raster owner。
        self.pipeline.read_pixels(x, y, width, height)
    }

    // 返回当前 logical width。
    fn width(&self) -> i32 {
        // 读取 WGL context 缓存的逻辑宽度。
        self.width
    }

    // 返回当前 logical height。
    fn height(&self) -> i32 {
        // 读取 WGL context 缓存的逻辑高度。
        self.height
    }

    // 清理当前 render target。
    fn clear_render_target(&mut self, r: f32, g: f32, b: f32, a: f32) -> Result<(), Error> {
        // 清理前确保 context current。
        self.make_current_result()?;
        // 委托给 OpenGL raster owner。
        self.pipeline.clear_render_target([r, g, b, a])
    }

    // 绘制 OpenGL ES 原生 solid rect batch。
    fn draw_solid_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给圆角 rect shader。
        self.pipeline
            .draw_solid_rects(viewport_w, viewport_h, scissor, rects)
    }

    // 绘制 OpenGL ES 原生 stroke rect batch。
    fn draw_stroke_rects(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuStrokeRect],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给共享圆角 SDF stroke shader。
        self.pipeline
            .draw_stroke_rects(viewport_w, viewport_h, scissor, rects)
    }

    // 绘制 OpenGL ES 原生线性渐变 batch。
    fn draw_linear_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        rects: &[GpuLinearGradientRect],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 legacy gradient compatibility owner。
        self.pipeline
            .draw_linear_gradients(viewport_w, viewport_h, scissor, rects)
    }

    // 绘制 OpenGL ES 原生径向渐变 batch。
    fn draw_radial_gradients(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        grads: &[GpuRadialGradient],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 legacy gradient compatibility owner。
        self.pipeline
            .draw_radial_gradients(viewport_w, viewport_h, scissor, grads)
    }

    // 绘制 OpenGL ES 原生 sector batch。
    fn draw_sectors(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        sectors: &[GpuSector],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 legacy sector compatibility owner。
        self.pipeline
            .draw_sectors(viewport_w, viewport_h, scissor, sectors)
    }

    // 绘制 OpenGL ES 原生 solid triangle mesh batch。
    fn draw_solid_meshes(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        meshes: &[GpuSolidMesh],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 legacy mesh compatibility owner。
        self.pipeline
            .draw_solid_meshes(viewport_w, viewport_h, scissor, meshes)
    }

    // 绘制 OpenGL ES 原生 box shadow batch。
    fn draw_box_shadows(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        shadows: &[GpuBoxShadow],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给共享仿射 SDF shadow shader。
        self.pipeline
            .draw_box_shadows(viewport_w, viewport_h, scissor, shadows)
    }

    // 绘制 OpenGL ES 原生图片 affine blit batch。
    fn draw_image_blits(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        blits: &[GpuImageBlit],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 legacy textured compatibility owner。
        self.pipeline
            .draw_image_blits(viewport_w, viewport_h, scissor, blits)
    }

    // 绘制 OpenGL ES 原生 glyph batch。
    fn draw_glyphs(
        &mut self,
        viewport_w: f32,
        viewport_h: f32,
        scissor: Option<(i32, i32, i32, i32)>,
        glyphs: &[GpuGlyphBlit],
    ) -> Result<(), Error> {
        // 确保 draw 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 glyph atlas owner。
        self.pipeline
            .draw_glyphs(viewport_w, viewport_h, scissor, glyphs)
    }

    // 将 bounded soft fallback tile 覆盖到当前 target。
    fn blit_soft_fallback_tile(
        &mut self,
        pixels: &[u32],
        tile: SoftFallbackTile,
    ) -> Result<(), Error> {
        // 确保 upload 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 读取当前 target 的逻辑尺寸。
        let (target_width, target_height) = self.pipeline.current_target_size();
        // 委托给 OpenGL soft texture owner。
        self.pipeline
            .blit_soft_fallback_tile(pixels, target_width, target_height, tile)
    }

    // 将完整 CPU surface 像素替换上传到当前 target。
    fn upload_surface_pixels(
        &mut self,
        pixels: &[u32],
        width: i32,
        height: i32,
    ) -> Result<(), Error> {
        // 确保 upload 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL soft texture owner。
        self.pipeline.upload_surface_pixels(pixels, width, height)
    }

    // 清理当前 target 的 bounded rects。
    fn clear_rects(
        &mut self,
        _viewport_w: f32,
        _viewport_h: f32,
        rects: &[GpuSolidRect],
    ) -> Result<(), Error> {
        // 确保 clear 发生在创建 context 的 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL clear owner。
        self.pipeline.clear_rects(rects)
    }

    // 创建 Picture 离屏 render target。
    fn create_offscreen_target(
        &mut self,
        width: i32,
        height: i32,
    ) -> Result<OffscreenTargetId, Error> {
        // 确保资源创建发生在 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL framebuffer owner。
        self.pipeline.create_offscreen_target(width, height)
    }

    // 销毁 Picture 离屏 render target。
    fn try_destroy_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        // 确保资源销毁发生在 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL framebuffer owner。
        self.pipeline.destroy_offscreen_target(id)
    }

    // 绑定 Picture 离屏 render target。
    fn bind_offscreen_target(&mut self, id: OffscreenTargetId) -> Result<(), Error> {
        // 确保绑定发生在 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL framebuffer owner。
        self.pipeline.bind_offscreen_target(id)
    }

    // 绑定 WGL swapchain render target。
    fn bind_swapchain_target(&mut self) -> Result<(), Error> {
        // 确保绑定发生在 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL framebuffer owner。
        self.pipeline.bind_swapchain_target();
        // 绑定操作无额外失败分支。
        Ok(())
    }

    // 将 Picture 离屏 texture blit 到当前 target。
    fn blit_offscreen_target(
        &mut self,
        id: OffscreenTargetId,
        src: crate::core::Rect,
        dst: crate::core::Rect,
        opacity: f32,
        additive: bool,
    ) -> Result<(), Error> {
        // 确保 blit 发生在 owner thread。
        self.make_current_result()?;
        // 委托给 OpenGL framebuffer texture owner。
        self.pipeline
            .blit_offscreen_target(id, src, dst, opacity, additive)
    }

    // 返回当前 WGL device pixel ratio。
    fn device_pixel_ratio(&self) -> f32 {
        // 逻辑宽度无效时使用安全默认值。
        if self.logical_width <= 0 {
            // 避免除零并保持旧兼容语义。
            return 1.0;
        }
        // 由 drawable 与逻辑宽度求出当前 ratio。
        self.width as f32 / self.logical_width as f32
    }
}
