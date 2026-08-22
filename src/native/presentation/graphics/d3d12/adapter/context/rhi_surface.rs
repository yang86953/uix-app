//! D3D12 swapchain 到 platform 通用 GraphicsSurface 的机械映射。
//!
//! 本模块只声明并执行 Surface 原语；完整 GraphicsDevice 与生产 registry 激活
//! 仍由后续阶段负责，缺失 Device 签发的提交会被共享 present 门禁前置拒绝。

// 引入稳定结果类型。
use crate::core::Result;
// 引入通用 Surface 角色、事务与事实型能力快照。
use crate::platform::presentation::rhi::{
    GraphicsSurface, GraphicsSurfaceCapabilities, RhiExtent, RhiPresentTransaction, RhiScissor,
    RhiSurfaceReadback, RhiSurfaceResizeTransaction, SurfaceFrame, SurfaceToken,
};

// D3D12 context 只机械消费共享 Surface 合同，不取得完整 GraphicsDevice 角色。
impl GraphicsSurface for super::D3d12Context {
    // 返回与 flip-discard 原生形态及同步回读实现一致的事实快照。
    fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities {
        // FullOnly 与候选 context 快照共用唯一 D3D12 原生事实。
        GraphicsSurfaceCapabilities::with_readback(super::super::D3D12_PRESENT_COHERENCY)
    }

    // 返回共享生命周期已经发布的唯一 Surface token。
    fn token(&self) -> SurfaceToken {
        self.surface_lifecycle.token()
    }

    // 获取当前隐式 D3D12 backbuffer 身份，不提交命令或触发 DXGI 调用。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // 故障与 shutdown 门禁必须发生在任何原生副作用前。
        self.ensure_healthy()?;
        // 共享生命周期失效后只能经 resize 重建，不得继续 acquire 旧代际。
        self.surface_lifecycle.ensure_active()?;
        // 只读取构造或 present 后冻结的 backbuffer 索引。
        if self.frame_index >= self.back_buffers.len() {
            let error = super::platform_error(format!(
                "D3d12Context: acquire frame index {} has only {} buffers",
                self.frame_index,
                self.back_buffers.len()
            ));
            // 内部 owner 形态不一致后禁止继续录制、回读或 resize。
            self.latch_fault("surface acquire", &error);
            return Err(error);
        }
        Ok(SurfaceFrame::new(self.token()))
    }

    // 按物理 extent 复用 recipe 与原生 adapter 的唯一 resize 事务。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // 在换算逻辑尺寸或触碰原生对象前完成健康门禁。
        self.ensure_healthy()?;
        let current = self.token();
        // 统一值域门禁签发一次封闭 resize 值，禁止 adapter 静默归一。
        let resize = RhiSurfaceResizeTransaction::validate(extent, current)?;
        // 逻辑尺寸只用于成功后的 PresentSurface 快照，物理请求保持原值进入原生事务。
        let dpr = current.extent.width as f32 / self.logical_width.max(1) as f32;
        let logical_width = (resize.extent().width as f32 / dpr.max(0.0001))
            .round()
            .max(1.0) as i32;
        let logical_height = (resize.extent().height as f32 / dpr.max(0.0001))
            .round()
            .max(1.0) as i32;
        self.run_surface_resize(resize, logical_width, logical_height)
    }

    // 同步读取当前 backbuffer，并返回通用左上原点 0xAARRGGBB 紧密像素。
    fn read_surface_pixels(&mut self, region: RhiScissor) -> Result<RhiSurfaceReadback> {
        // shutdown 与已锁存原生故障必须先于任何 D3D12 资源访问拒绝。
        self.ensure_healthy()?;
        // 已回滚的 Surface 不得继续把旧 token 当成可回读事实。
        self.surface_lifecycle.ensure_active()?;
        let token = self.token();
        // 共享范围门禁先于 staging buffer、命令录制、拷贝和 Map。
        RhiSurfaceReadback::validate_region(region, token.extent)?;
        let pixels = self.read_pixels_result(region.x, region.y, region.width, region.height)?;
        // 复用通用长度与区域后置检查，不复制 readback 语义。
        RhiSurfaceReadback::try_new(region, token.extent, pixels)
    }

    // 只把通过共享 frame、submission 与 damage 门禁的事务交给 DXGI。
    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        // 故障与 shutdown 门禁不得被缺失 Device 的提交拒绝掩盖。
        self.ensure_healthy()?;
        // 已失效代际必须在 submission 门禁和 DXGI 前拒绝。
        self.surface_lifecycle.ensure_active()?;
        // D3D12 显式命令批次必须已经成功 submit，禁止旧身份呈现未提交内容。
        self.rhi_require_present_ready()?;
        let current = self.token();
        let coherency = self.surface_capabilities().present_coherency;
        // 只接受同一资源 Component 在真实 D3D12 提交成功后签发的最新身份。
        let present = transaction.validate(current, coherency, self.rhi_device.submissions())?;
        // 通过共享事务后才进入现有 checked present 与故障锁存路径。
        self.present_result(&present)
    }
}
