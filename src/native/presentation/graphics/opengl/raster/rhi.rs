//! OpenGlRasterPipeline 到薄 RHI 的 owner-thread bridge。

// 引入统一错误和 RHI 原语。
use crate::core::error::Result;
// 引入 Surface 冻结的跨呈现像素保留语义。
use crate::core::PresentCoherency;
use crate::native::present::rhi::{
    BufferDesc, BufferHandle, DrawPacket, GraphicsDeviceCapabilities, LoadAction, PipelineBinding,
    PipelineDesc, RenderTargetHandle, RhiBufferUpload, RhiExtent, RhiPresentTransaction,
    RhiScissor, RhiTextureUpload, RhiViewport, SampledTextureBinding, SamplerDesc, SamplerHandle,
    SubmissionHandle, SurfaceToken, TextureCopy, TextureDesc, TextureHandle, TextureMove,
    ValidatedRhiPresent,
};
// 复用父模块中的 OpenGL pipeline 和资源设备类型。
use super::{OpenGlRasterPipeline, rhi_device::OpenGlRhiDevice};

// 为 OpenGlRasterPipeline 提供通用 RHI 资源和命令转发。
impl OpenGlRasterPipeline {
    // 同时借用 runtime 的 GL context 和独立 RHI 资源表，避免交叉可变借用。
    fn with_rhi<T>(
        &mut self,
        operation: impl FnOnce(&glow::Context, &mut OpenGlRhiDevice) -> T,
    ) -> T {
        // 两个字段属于同一个 owner，但生命周期和可变性彼此独立。
        let runtime = &self.runtime;
        let rhi = &mut self.rhi;
        operation(runtime.context(), rhi)
    }

    // 返回 OpenGL ES RHI 的事实能力快照。
    pub(crate) fn rhi_device_capabilities(&self) -> GraphicsDeviceCapabilities {
        // GLES 3.0 已实现通用 Renderer 需要的完整 Device 原语基线。
        let mut capabilities = GraphicsDeviceCapabilities::full_gpu_baseline();
        // scratch texture 使同纹理重叠移动具有确定的 memmove 语义。
        capabilities.texture_region_move = true;
        // OpenGL scissor clear 已由 RHI owner 执行并恢复状态。
        capabilities.clear_rect = true;
        // 返回只包含资源、pass 与 pipeline 原语的 Device 事实快照。
        capabilities
    }

    // 创建通用动态 buffer。
    pub(crate) fn rhi_create_buffer(&mut self, desc: BufferDesc) -> Result<BufferHandle> {
        // 资源创建使用已经由 native context current 的 GL runtime。
        self.with_rhi(|gl, rhi| rhi.create_buffer(gl, desc))
    }

    // 更新通用 buffer。
    pub(crate) fn rhi_update_buffer(&mut self, upload: RhiBufferUpload<'_>) -> Result<()> {
        // 上传与 CPU uniform 镜像由同一个 RHI owner 完成。
        self.with_rhi(|gl, rhi| rhi.update_buffer(gl, upload))
    }

    // 创建通用 texture。
    pub(crate) fn rhi_create_texture(&mut self, desc: TextureDesc) -> Result<TextureHandle> {
        // 颜色 texture 同时拥有 sampled view 和 offscreen framebuffer。
        self.with_rhi(|gl, rhi| rhi.create_texture(gl, desc))
    }

    // 上传通用 texture payload。
    pub(crate) fn rhi_update_texture(&mut self, upload: RhiTextureUpload<'_>) -> Result<()> {
        // 保持目标身份、区域与载荷绑定到 owner-thread Adapter 边界。
        self.with_rhi(|gl, rhi| rhi.update_texture(gl, upload))
    }

    // 创建通用 sampler。
    pub(crate) fn rhi_create_sampler(&mut self, desc: SamplerDesc) -> Result<SamplerHandle> {
        // sampler 过滤和边界状态只由通用 descriptor 决定。
        self.with_rhi(|gl, rhi| rhi.create_sampler(gl, desc))
    }

    // 创建封闭语义的通用 pipeline。
    pub(crate) fn rhi_create_pipeline(&mut self, desc: PipelineDesc) -> Result<PipelineBinding> {
        // shader 编译失败必须在首次 frame 前返回 typed error。
        self.with_rhi(|gl, rhi| rhi.create_pipeline(gl, desc))
    }

    // 销毁通用 buffer。
    pub(crate) fn rhi_destroy_buffer(&mut self, buffer: BufferHandle) -> Result<()> {
        // 资源删除使用当前 GL context，保持 owner-thread 生命周期。
        self.with_rhi(|gl, rhi| rhi.destroy_buffer(gl, buffer))
    }

    // 销毁通用 texture。
    pub(crate) fn rhi_destroy_texture(&mut self, texture: TextureHandle) -> Result<()> {
        // 同时删除颜色 texture 的离屏 framebuffer。
        self.with_rhi(|gl, rhi| rhi.destroy_texture(gl, texture))
    }

    // 销毁通用 sampler。
    pub(crate) fn rhi_destroy_sampler(&mut self, sampler: SamplerHandle) -> Result<()> {
        // 删除 sampler 前由资源表校验其当前身份。
        self.with_rhi(|gl, rhi| rhi.destroy_sampler(gl, sampler))
    }

    // 销毁通用 pipeline。
    pub(crate) fn rhi_destroy_pipeline(&mut self, pipeline: PipelineBinding) -> Result<()> {
        // 删除已经编译的 GLES program。
        self.with_rhi(|gl, rhi| rhi.destroy_pipeline(gl, pipeline))
    }

    // 开始一个 surface 或离屏 render pass。
    pub(crate) fn rhi_begin_render_pass(
        &mut self,
        target: RenderTargetHandle,
        load: LoadAction,
    ) -> Result<()> {
        // swapchain 的 physical extent 是 drawable 而非 logical extent。
        let surface_extent = RhiExtent::new(
            self.swapchain.drawable_width.max(1) as u32,
            self.swapchain.drawable_height.max(1) as u32,
        );
        // 让设备解析 surface sentinel 或 texture target。
        self.with_rhi(|gl, rhi| rhi.begin_render_pass(gl, target, load, surface_extent))
    }

    // 设置当前 pass 的 viewport。
    pub(crate) fn rhi_set_viewport(&mut self, viewport: RhiViewport) -> Result<()> {
        // viewport 的逻辑和 Y 翻转由 RHI device 统一处理。
        self.with_rhi(|gl, rhi| rhi.set_viewport(gl, viewport))
    }

    // 设置当前 pass 的 scissor。
    pub(crate) fn rhi_set_scissor(&mut self, scissor: Option<RhiScissor>) -> Result<()> {
        // 将左上原点 scissor 转换为 GLES bottom-left 原点。
        self.with_rhi(|gl, rhi| rhi.set_scissor(gl, scissor))
    }

    // 在当前 pass 内清理一个物理矩形。
    pub(crate) fn rhi_clear_rect(
        &mut self,
        color: crate::native::present::rhi::RhiColor,
        scissor: RhiScissor,
    ) -> Result<()> {
        // 让 OpenGL RHI helper 临时覆盖并恢复当前 scissor。
        self.with_rhi(|gl, rhi| rhi.clear_rect(gl, color, scissor))
    }

    // 绑定当前 pass 的 sampled texture。
    pub(crate) fn rhi_bind_sampled_texture(
        &mut self,
        binding: SampledTextureBinding,
    ) -> Result<()> {
        // 纹理和 sampler 的真实 GL 绑定延迟到 draw，保留 command order。
        self.with_rhi(|_, rhi| rhi.bind_sampled_texture(binding))
    }

    // 执行一个通用 draw packet。
    pub(crate) fn rhi_draw(&mut self, packet: DrawPacket) -> Result<()> {
        // draw ABI 分派由 OpenGlRhiDevice 子模块负责。
        self.with_rhi(|gl, rhi| rhi.draw(gl, packet))
    }

    // 在 pass 外复制 texture。
    pub(crate) fn rhi_copy_texture(&mut self, copy: TextureCopy) -> Result<()> {
        // 复制使用源 framebuffer 和目标 texture 的 GLES 3.0 原语。
        self.with_rhi(|gl, rhi| rhi.copy_texture(gl, copy))
    }

    // 在 OpenGL RHI 中执行重叠安全的 texture region move。
    pub(crate) fn rhi_move_texture_region(&mut self, movement: TextureMove) -> Result<()> {
        // scratch texture 的创建、两段 copy 和销毁都留在同一 owner thread。
        self.with_rhi(|gl, rhi| rhi.move_texture_region(gl, movement))
    }

    // 结束当前 render pass。
    pub(crate) fn rhi_end_render_pass(&mut self) -> Result<()> {
        // 解除 framebuffer、VAO 和 sampled resource 绑定。
        self.with_rhi(|gl, rhi| rhi.end_render_pass(gl))
    }

    // 提交当前 device 命令并返回 submit identity。
    pub(crate) fn rhi_submit(&mut self) -> Result<SubmissionHandle> {
        // OpenGL immediate context 通过 flush 划定提交边界。
        self.with_rhi(|gl, rhi| rhi.submit(gl))
    }

    // 执行 OpenGL RHI 的 owner-thread 健康维护。
    pub(crate) fn rhi_maintain(&mut self) -> Result<()> {
        // 健康检查只访问 RHI 状态，不创建或提交资源。
        self.rhi.maintain()
    }

    // 安排一次 test-harness 设备丢失。
    #[cfg(feature = "test-harness")]
    pub(crate) fn rhi_inject_device_lost_for_test(&mut self) -> Result<()> {
        // 让共享 OpenGL RHI 设备保存一次性故障标记。
        self.rhi.arm_device_lost_for_test();
        Ok(())
    }

    // 安排一次 test-harness surface 丢失。
    #[cfg(feature = "test-harness")]
    pub(crate) fn rhi_inject_surface_lost_for_test(&mut self) -> Result<()> {
        // 让共享 OpenGL surface host 在 acquire/present 边界消费故障。
        self.rhi.arm_surface_lost_for_test();
        Ok(())
    }

    // 消费一次 test-harness surface 丢失。
    #[cfg(feature = "test-harness")]
    pub(crate) fn rhi_take_surface_lost_for_test(&mut self) -> bool {
        // 返回并清除一次性故障标记。
        self.rhi.take_surface_lost_for_test()
    }

    // 通过共享门禁检查一次不可拆的 Surface 呈现事务。
    pub(crate) fn rhi_validate_present(
        // 只读借用 owner-thread pipeline。
        &self,
        // 接收 FramePlan 构造的完整呈现事务。
        transaction: RhiPresentTransaction,
        // 接收宿主当前 drawable token。
        current_token: SurfaceToken,
        // 接收宿主 Surface capability 的保留事实。
        present_coherency: PresentCoherency,
    ) -> Result<ValidatedRhiPresent> {
        // 只向 Device Component 传递动态事实，不在 bridge 重复解释规则。
        self.rhi
            // 共享门禁同时核对 frame、目标和最近提交。
            .validate_present(transaction, current_token, present_coherency)
    }

    // 返回当前 swapchain 的物理 extent。
    pub(crate) fn rhi_surface_extent(&self) -> RhiExtent {
        // surface token 使用 drawable 尺寸，和 RHI viewport ABI 一致。
        RhiExtent::new(
            self.swapchain.drawable_width.max(1) as u32,
            self.swapchain.drawable_height.max(1) as u32,
        )
    }

    // 在 native context shutdown 前释放所有 RHI 资源。
    pub(crate) fn rhi_release(&mut self) {
        // 删除 RHI 对象前保证调用方仍持有 current GLES context。
        self.with_rhi(|gl, rhi| rhi.release(gl));
    }
}
