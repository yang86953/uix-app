//! WGL/EGL context 到 OpenGL ES 薄 RHI 的共享 host 实现。

// 引入最终 present damage、保留证明和通用结果类型。
use crate::core::{PresentCoherency, PresentDamage, Result};
// 仅 test-harness surface lost 注入需要专用错误分类和构造器。
#[cfg(feature = "test-harness")]
use crate::core::{Errc, Error};
// 引入薄 RHI 的所有组合 trait 与命令类型。
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsDevice, GraphicsDeviceCapabilities,
    GraphicsSurface, GraphicsSurfaceCapabilities, LoadAction, PipelineBinding, PipelineDesc,
    RenderTargetHandle, RhiBufferUpload, RhiColor, RhiExtent, RhiPresentTransaction, RhiScissor,
    RhiSurfaceReadback, RhiSurfaceResizeTransaction, RhiTextureUpload, RhiViewport,
    SampledTextureBinding, SamplerDesc, SamplerHandle, SubmissionHandle, SurfaceFrame,
    SurfaceToken, TextureCopy, TextureDesc, TextureHandle, TextureMove,
};
// 引入 OpenGL surface test_present 的共享结果类型。
use crate::native::present::PresentTestResult;
// 引入 OpenGL raster pipeline 的 RHI bridge。
use super::raster::OpenGlRasterPipeline;

// 描述 WGL/EGL 共用的 OpenGL ES context host 生命周期。
pub(crate) trait OpenGlRhiHost {
    // 检查 OpenGL owner 是否仍允许任何 RHI 生命周期操作。
    fn rhi_ensure_active(&self) -> Result<()>;
    // 借用可变的 raster/RHI owner。
    fn rhi_pipeline_mut(&mut self) -> &mut OpenGlRasterPipeline;
    // 借用只读的 raster/RHI owner。
    fn rhi_pipeline(&self) -> &OpenGlRasterPipeline;
    // 让后续 RHI GL 命令在 owner thread 的 native context 上执行。
    fn rhi_make_current(&mut self) -> Result<()>;
    // 读取当前 surface generation。
    fn rhi_generation(&self) -> u64;
    // 消费已验证事务并按物理 extent 重建宿主 surface。
    fn rhi_resize_surface(&mut self, resize: RhiSurfaceResizeTransaction) -> Result<()>;
    // 将最近一次 RHI submit 交换到原生窗口。
    fn rhi_swap_buffers(&mut self, damage: PresentDamage) -> Result<()>;
}

// 将共享 RHI device 原语转发给 WGL/EGL context。
impl<T> GraphicsDevice for T
where
    T: OpenGlRhiHost,
{
    // 返回 OpenGL ES RHI 的事实能力。
    fn device_capabilities(&self) -> GraphicsDeviceCapabilities {
        self.rhi_pipeline().rhi_device_capabilities()
    }

    // 激活当前 OpenGL owner 的原生 context，不把 thread-current 事实泄漏给 Drawing。
    fn activate(&mut self) -> Result<()> {
        // 在恢复 native context 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 所有资源、命令和释放操作都通过同一 Adapter 入口恢复 current context。
        self.rhi_make_current()
    }

    // 在已经 current 的 OpenGL context 上消费设备健康检查。
    fn maintain(&mut self) -> Result<()> {
        // 在读取设备健康状态前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 健康检查不再承担隐式 context 激活，避免 checked teardown 被 DeviceLost 阻断。
        self.rhi_pipeline_mut().rhi_maintain()
    }

    // 安排一次真实 OpenGL adapter 边界上的 test-harness device lost。
    #[cfg(feature = "test-harness")]
    fn inject_device_lost_for_test(&mut self) -> Result<()> {
        // 在安排测试故障前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 故障标记由共享 RHI pipeline 保存，恢复层仍消费 typed error。
        self.rhi_pipeline_mut().rhi_inject_device_lost_for_test()
    }

    // 创建动态 buffer。
    fn create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        // 在创建 native buffer 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_create_buffer(desc)
    }

    // 更新动态 buffer。
    fn update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 在上传 native buffer 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 保持句柄与载荷绑定，避免 host bridge 重新拆散上传语义。
        self.rhi_pipeline_mut().rhi_update_buffer(upload)
    }

    // 创建 sampled 或 render target texture。
    fn create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        // 在创建 native texture 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_create_texture(desc)
    }

    // 上传 texture payload。
    fn update_texture(&mut self, upload: RhiTextureUpload<'_>) -> Result<()> {
        // 在上传 native texture 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 让 OpenGL host 保持上传命令完整交给 owner-thread raster pipeline。
        self.rhi_pipeline_mut().rhi_update_texture(upload)
    }

    // 创建 sampler。
    fn create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        // 在创建 native sampler 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_create_sampler(desc)
    }

    // 创建固定 pipeline。
    fn create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        // 在创建 native pipeline 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 让 OpenGL raster owner 直接返回共享表签发的完整身份。
        self.rhi_pipeline_mut().rhi_create_pipeline(desc)
    }

    // 销毁 buffer。
    fn destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        // 在释放 native buffer 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_destroy_buffer(buffer)
    }

    // 销毁 texture。
    fn destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        // 在释放 native texture 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_destroy_texture(texture)
    }

    // 销毁 sampler。
    fn destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        // 在释放 native sampler 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_destroy_sampler(sampler)
    }

    // 销毁 pipeline。
    fn destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        // 在释放 native pipeline 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 让 OpenGL raster owner 校验完整绑定后销毁原生 program。
        self.rhi_pipeline_mut().rhi_destroy_pipeline(pipeline)
    }

    // 开始 render pass。
    fn begin_render_pass(&mut self, target: RenderTargetHandle, load: LoadAction) -> Result<()> {
        // 在打开 native pass 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_begin_render_pass(target, load)
    }

    // 设置 viewport。
    fn set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        // 在写入 native viewport 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_set_viewport(viewport)
    }

    // 设置 scissor。
    fn set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        // 在写入 native scissor 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_set_scissor(scissor)
    }

    // 在当前 OpenGL pass 内清理一个物理矩形。
    fn clear_rect(&mut self, color: RhiColor, scissor: RhiScissor) -> Result<()> {
        // 在清理 native target 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 将局部清理交给 owner-thread raster RHI。
        self.rhi_pipeline_mut().rhi_clear_rect(color, scissor)
    }

    // 绑定完整 sampled texture 事实。
    fn bind_sampled_texture(&mut self, binding: SampledTextureBinding) -> Result<()> {
        // 在绑定 native sampled resource 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_bind_sampled_texture(binding)
    }

    // 执行 draw packet。
    fn draw(&mut self, packet: DrawPacket) -> Result<()> {
        // 在执行 native draw 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_draw(packet)
    }

    // 执行 texture copy。
    fn copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        // 在执行 native texture copy 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_copy_texture(copy)
    }

    // 执行具有重叠安全语义的 texture region move。
    fn move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // 在执行 native texture move 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 将 retained framebuffer 移动交给 OpenGL RHI owner。
        self.rhi_pipeline_mut().rhi_move_texture_region(movement)
    }

    // 结束 render pass。
    fn end_render_pass(&mut self) -> Result<()> {
        // 在关闭 native pass 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_end_render_pass()
    }

    // 提交当前命令序列。
    fn submit(&mut self) -> Result<SubmissionHandle> {
        // 在提交 native command stream 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        self.rhi_pipeline_mut().rhi_submit()
    }
}

// 将共享 surface 生命周期转发给 WGL/EGL context。
impl<T> GraphicsSurface for T
where
    T: OpenGlRhiHost,
{
    // 返回 OpenGL window surface 实际实现的可选能力。
    fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities {
        // WGL 与 EGL 当前都只承诺完整交换，同时实现同步回读原语。
        GraphicsSurfaceCapabilities::with_readback(
            // 没有 buffer-age 或 swap-with-damage 证明时必须保守保持 FullOnly。
            PresentCoherency::FullOnly,
        )
    }

    // 返回当前代际和物理 surface extent。
    fn token(&self) -> SurfaceToken {
        SurfaceToken::new(
            self.rhi_generation(),
            self.rhi_pipeline().rhi_surface_extent(),
        )
    }

    // 获取当前 swapchain target，并确保 GL context current。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // 在取得 native surface frame 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
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
        Ok(SurfaceFrame::new(self.token()))
    }

    // 按物理 extent 重建宿主 surface 并返回新代际 token。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // 在进入 native surface resize 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 在进入 EGL 或 WGL 前冻结旧 token 并执行唯一共同值域验证。
        let resize = RhiSurfaceResizeTransaction::validate(extent, self.token())?;
        // 原生 host 只能消费字段封闭的已验证事务。
        self.rhi_resize_surface(resize)?;
        // 发布前统一验证请求 extent 与 generation 后置条件。
        resize.complete(self.token())
    }

    // 读取当前 OpenGL drawable 并返回左上原点 0xAARRGGBB 规范结果。
    fn read_surface_pixels(&mut self, region: RhiScissor) -> Result<RhiSurfaceReadback> {
        // 在读取 native surface 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 在进入 OpenGL 前使用共享范围规则拒绝越界或空请求。
        RhiSurfaceReadback::validate_region(region, self.token().extent)?;
        // 回读前恢复 owner-thread current context。
        self.rhi_make_current()?;
        // 委托给共享 OpenGL raster owner。
        let pixels = self.rhi_pipeline_mut().read_pixels(
            // 传入共享契约已验证的横坐标。
            region.x,
            // 传入共享契约已验证的纵坐标。
            region.y,
            // 传入共享契约已验证的宽度。
            region.width,
            // 传入共享契约已验证的高度。
            region.height,
        )?;
        // OpenGL raster 已完成行序与通道规范化，集中验证载荷后返回。
        RhiSurfaceReadback::try_new(region, self.token().extent, pixels)
    }

    // 只接受通过共享门禁的 Surface 呈现事务。
    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        // 在执行 native present 前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
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
        // 在原生交换前读取当前 drawable 的代际与 extent。
        let current_token = self.token();
        // 从当前 OpenGL Surface profile 冻结完整交换能力。
        let present_coherency = self.surface_capabilities().present_coherency;
        // 通过共享门禁验证 frame、目标和最新提交关联。
        let present = self.rhi_pipeline().rhi_validate_present(
            // 交付不可拆的 FramePlan 呈现事务。
            transaction,
            // 传入当前宿主 Surface token。
            current_token,
            // 传入同一 Surface profile 的 FullOnly 事实。
            present_coherency,
        )?;
        // Adapter 只消费门禁发布的 damage 执行原生交换。
        self.rhi_swap_buffers(present.into_damage())
    }

    // OpenGL 当前没有 surface 可恢复性探测，但关闭后必须优先拒绝调用。
    fn test_present(&mut self) -> Result<PresentTestResult> {
        // 在返回稳定未实现错误前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 保持 OpenGL surface 的明确未实现语义。
        Err(crate::core::Error::new(
            crate::core::Errc::NotImplemented,
            "OpenGL surface test_present is not implemented",
        ))
    }

    // OpenGL surface 维护不执行额外操作，但仍受 owner 生命周期门禁保护。
    fn maintain(&mut self) -> Result<()> {
        // 在 surface 维护入口先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 健康检查由 GraphicsDevice::maintain 负责，surface 维护保持无操作。
        Ok(())
    }

    // 安排一次真实 OpenGL adapter 边界上的 test-harness surface lost。
    #[cfg(feature = "test-harness")]
    fn inject_surface_lost_for_test(&mut self) -> Result<()> {
        // 在安排 surface 测试故障前先拒绝已关闭 owner。
        self.rhi_ensure_active()?;
        // 故障标记由共享 RHI pipeline 保存，恢复层仍消费 typed error。
        self.rhi_pipeline_mut().rhi_inject_surface_lost_for_test()
    }
}
