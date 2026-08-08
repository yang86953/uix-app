//! WGL/EGL context 到 OpenGL ES 薄 RHI 的共享 host 实现。

// 引入最终 present damage 和通用 typed error。
use crate::core::{Error, PresentDamage, Result};
// 仅 test-harness surface lost 注入需要专用错误分类。
#[cfg(feature = "test-harness")]
use crate::core::Errc;
// 引入薄 RHI 的所有组合 trait 与命令类型。
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsCapabilities, GraphicsDevice, GraphicsSurface,
    LoadAction, PipelineDesc, PipelineHandle, RenderTargetHandle, RhiColor, RhiExtent, RhiScissor,
    RhiViewport, SamplerDesc, SamplerHandle, SubmissionHandle, SurfaceFrame, SurfaceToken,
    TextureCopy, TextureDesc, TextureHandle, TextureMove,
};
// 引入 OpenGL raster pipeline 的 RHI bridge。
use super::raster::OpenGlRasterPipeline;

// 描述 WGL/EGL 共用的 OpenGL ES context host 生命周期。
pub(crate) trait OpenGlRhiHost {
    // 借用可变的 raster/RHI owner。
    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline;
    // 借用只读的 raster/RHI owner。
    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline;
    // 让后续 RHI GL 命令在 owner thread 的 native context 上执行。
    fn rhi_make_current(&mut self) -> Result<()>;
    // 读取当前 surface generation。
    fn rhi_generation(&self) -> u64;
    // 按物理 extent 重建宿主 surface。
    fn rhi_resize_surface(&mut self, extent: RhiExtent) -> Result<()>;
    // 将最近一次 RHI submit 交换到原生窗口。
    fn rhi_swap_buffers(&mut self, damage: PresentDamage) -> Result<()>;
}

// 将共享 RHI device 原语转发给 WGL/EGL context。
impl<T> GraphicsDevice for T
where
    T: OpenGlRhiHost,
{
    // 返回 OpenGL ES RHI 的事实能力。
    fn capabilities(&self) -> GraphicsCapabilities {
        self.rhi_pipeline().rhi_capabilities()
    }

    // 在最终提交前消费 OpenGL RHI 的 owner-thread 健康检查。
    fn maintain(&mut self) -> Result<()> {
        // 维护状态不进入高层兼容绘制入口。
        self.rhi_pipeline_mut().rhi_maintain()
    }

    // 安排一次真实 OpenGL adapter 边界上的 test-harness device lost。
    #[cfg(feature = "test-harness")]
    fn inject_device_lost_for_test(&mut self) -> Result<()> {
        // 故障标记由共享 RHI pipeline 保存，恢复层仍消费 typed error。
        self.rhi_pipeline_mut().rhi_inject_device_lost_for_test()
    }

    // 创建动态 buffer。
    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        self.rhi_pipeline_mut().rhi_create_buffer(desc)
    }

    // 更新动态 buffer。
    fn update_buffer(&mut self, buffer: BufferHandle, offset: usize, data: &[u8]) -> Result<()> {
        self.rhi_pipeline_mut()
            .rhi_update_buffer(buffer, offset, data)
    }

    // 创建 sampled 或 render target texture。
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        self.rhi_pipeline_mut().rhi_create_texture(desc)
    }

    // 上传 texture payload。
    fn update_texture(
        &mut self,
        texture: TextureHandle,
        extent: RhiExtent,
        data: &[u8],
    ) -> Result<()> {
        self.rhi_pipeline_mut()
            .rhi_update_texture(texture, extent, data)
    }

    // 上传通用 texture 的带偏移子区域，供通用 atlas 复用同一张纹理。
    fn update_texture_region(
        &mut self,
        texture: TextureHandle,
        destination_x: u32,
        destination_y: u32,
        extent: RhiExtent,
        data: &[u8],
    ) -> Result<()> {
        // 让 OpenGL host 继续把资源操作交给 owner-thread raster pipeline。
        self.rhi_pipeline_mut().rhi_update_texture_region(
            texture,
            destination_x,
            destination_y,
            extent,
            data,
        )
    }

    // 创建 sampler。
    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        self.rhi_pipeline_mut().rhi_create_sampler(desc)
    }

    // 创建固定 pipeline。
    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineHandle> {
        self.rhi_pipeline_mut().rhi_create_pipeline(desc)
    }

    // 销毁 buffer。
    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        self.rhi_pipeline_mut().rhi_destroy_buffer(buffer)
    }

    // 销毁 texture。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        self.rhi_pipeline_mut().rhi_destroy_texture(texture)
    }

    // 销毁 sampler。
    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        self.rhi_pipeline_mut().rhi_destroy_sampler(sampler)
    }

    // 销毁 pipeline。
    fn destroy_pipeline(&mut self, pipeline: PipelineHandle) -> Result<()> {
        self.rhi_pipeline_mut().rhi_destroy_pipeline(pipeline)
    }

    // 开始 render pass。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        self.rhi_pipeline_mut().rhi_begin_render_pass(target, load)
    }

    // 设置 viewport。
    fn set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        self.rhi_pipeline_mut().rhi_set_viewport(viewport)
    }

    // 设置 scissor。
    fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        self.rhi_pipeline_mut().rhi_set_scissor(scissor)
    }

    // 在当前 OpenGL pass 内清理一个物理矩形。
    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        // 将局部清理交给 owner-thread raster RHI。
        self.rhi_pipeline_mut().rhi_clear_rect(color, scissor)
    }

    // 绑定 sampled texture。
    fn bind_texture(
        &mut self,
        slot: u32,
        texture: TextureHandle,
        sampler: SamplerHandle,
    ) -> Result<()> {
        self.rhi_pipeline_mut()
            .rhi_bind_texture(slot, texture, sampler)
    }

    // 执行 draw packet。
    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        self.rhi_pipeline_mut().rhi_draw(packet)
    }

    // 执行 texture copy。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        self.rhi_pipeline_mut().rhi_copy_texture(copy)
    }

    // 执行具有重叠安全语义的 texture region move。
    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // 将 retained framebuffer 移动交给 OpenGL RHI owner。
        self.rhi_pipeline_mut().rhi_move_texture_region(movement)
    }

    // 结束 render pass。
    fn end_render_pass(&mut self) -> Result<()> {
        self.rhi_pipeline_mut().rhi_end_render_pass()
    }

    // 提交当前命令序列。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        self.rhi_pipeline_mut().rhi_submit()
    }
}

// 将共享 surface 生命周期转发给 WGL/EGL context。
impl<T> GraphicsSurface for T
where
    T: OpenGlRhiHost,
{
    // 返回当前代际和物理 surface extent。
    fn token(&self) -> SurfaceToken {
        SurfaceToken::new(
            self.rhi_generation(),
            self.rhi_pipeline().rhi_surface_extent(),
        )
    }

    // 获取当前 swapchain target，并确保 GL context current。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        self.rhi_make_current()?;
        // 测试故障在真实 OpenGL surface acquire 边界返回 typed surface lost。
        #[cfg(feature = "test-harness")]
        if self.rhi_pipeline_mut().rhi_take_surface_lost_for_test() {
            // 记录稳定 lower marker，便于真窗恢复用例核对边界。
            tracing::warn!("OpenGL RHI test surface lost");
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "OpenGL RHI test surface lost before acquire",
            ));
        }
        Ok(SurfaceFrame::new(
            self.token(),
            RenderTargetHandle::from_raw(super::raster::OPENGL_RHI_SURFACE_TARGET_RAW),
        ))
    }

    // 按物理 extent 重建宿主 surface 并返回新代际 token。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        if !extent.is_positive() {
            return Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                "OpenGL RHI surface extent must be positive",
            ));
        }
        self.rhi_resize_surface(extent)?;
        Ok(self.token())
    }

    // 只接受当前代际、当前 surface target 和最近一次提交。
    fn present(
        &mut self,
        frame: SurfaceFrame,
        submission: SubmissionHandle,
        damage: PresentDamage,
    ) -> Result<()> {
        // 交换前再次 current，防止宿主的 legacy 操作改变 current context。
        self.rhi_make_current()?;
        // surface lost 必须在共享 adapter present 前消费，而不是静默交换。
        #[cfg(feature = "test-harness")]
        if self.rhi_pipeline_mut().rhi_take_surface_lost_for_test() {
            // 记录与 acquire 共用的 lower marker。
            tracing::warn!("OpenGL RHI test surface lost");
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "OpenGL RHI test surface lost before present",
            ));
        }
        if frame.token != self.token() {
            return Err(Error::new(
                crate::core::error::Errc::GraphicsSurfaceLost,
                "OpenGL RHI surface frame generation is stale",
            ));
        }
        if frame.target.raw() != super::raster::OPENGL_RHI_SURFACE_TARGET_RAW {
            return Err(Error::new(
                crate::core::error::Errc::InvalidArgument,
                "OpenGL RHI present requires the acquired surface target",
            ));
        }
        self.rhi_pipeline().rhi_validate_submission(submission)?;
        self.rhi_swap_buffers(damage)
    }

    // 安排一次真实 OpenGL adapter 边界上的 test-harness surface lost。
    #[cfg(feature = "test-harness")]
    fn inject_surface_lost_for_test(&mut self) -> Result<()> {
        // 故障标记由共享 RHI pipeline 保存，恢复层仍消费 typed error。
        self.rhi_pipeline_mut().rhi_inject_surface_lost_for_test()
    }
}
