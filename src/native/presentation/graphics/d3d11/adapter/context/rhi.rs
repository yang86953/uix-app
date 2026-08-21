//! D3D11 surface 对薄 RHI 的迁移期实现。
//!
//! 本文件只接入 surface 生命周期；低层 device/pass/draw 由同级 RHI device
//! 模块实现，UI 语义通过类型化 GPU recipe owner 消费。

// 引入统一结果类型。
use crate::core::error::Result;
// 仅 test-harness surface lost 注入需要错误分类和构造器。
#[cfg(feature = "test-harness")]
use crate::core::error::{Errc, Error};
// 引入 context 上已有的 DPR 快照与无帧探测结果。
use crate::native::present::{GraphicsContextLifecycle, PresentTestResult};
// 引入薄 RHI 的 surface 原语。
use crate::platform::presentation::rhi::{
    GraphicsSurface, GraphicsSurfaceCapabilities, RhiExtent, RhiPresentTransaction, RhiScissor,
    RhiSurfaceReadback, RhiSurfaceResizeTransaction, SurfaceFrame, SurfaceToken,
};

// 为 D3D11 context 实现 surface acquire/resize/present。
impl GraphicsSurface for super::D3d11Context {
    // 返回从实际 swapchain 形态投影的 Surface 可选能力。
    fn surface_capabilities(&self) -> GraphicsSurfaceCapabilities {
        // D3D11 Surface 同时报告真实 swapchain 保留证明与同步回读原语。
        GraphicsSurfaceCapabilities::with_readback(
            // 直接读取当前 Surface 自己拥有的冻结 swapchain 契约。
            self.present_coherency(),
        )
    }

    // 返回当前 D3D11 swapchain 的 surface 代际和物理 extent。
    fn token(&self) -> SurfaceToken {
        // 把 context 当前 drawable 尺寸映射为 RHI extent。
        SurfaceToken::new(
            self.surface_generation,
            RhiExtent::new(self.width.max(1) as u32, self.height.max(1) as u32),
        )
    }

    // 获取当前 backbuffer 的 opaque target，不在此处执行最终 present。
    fn acquire(&mut self) -> Result<SurfaceFrame> {
        // checked shutdown 后不得重新取得或创建 surface frame。
        self.ensure_active()?;
        // 测试注入在 acquire 边界返回 surface lost，保证不写入失效目标。
        #[cfg(feature = "test-harness")]
        if std::mem::take(&mut self.rhi_surface_lost_for_test) {
            // 只在 test-harness 记录 lower boundary，便于真实窗口测试定位。
            tracing::warn!("D3d11 RHI test surface lost");
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "D3d11 RHI test surface lost before acquire",
            ));
        }
        // 确保 backbuffer RTV 已经创建，避免 plan 在第一条 pass 才失败。
        self.ensure_rtv()?;
        // 返回当前代际和保留的 surface target 身份。
        Ok(SurfaceFrame::new(self.token()))
    }

    // 按物理 extent 重建 swapchain，并把输入转换回窗口逻辑尺寸。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // checked shutdown 后不得进入 surface resize 事务。
        self.ensure_active()?;
        // 在触碰 DXGI 前冻结旧 token 并执行唯一共同值域验证。
        let resize = RhiSurfaceResizeTransaction::validate(extent, self.token())?;
        // 读取共享事务已经证明安全的原生有符号尺寸。
        let (native_width, native_height) = resize.native_size_i32();
        // 使用当前 DPR 把物理尺寸转换为兼容 context 的逻辑尺寸。
        // 从单一 surface 快照读取 DPR，避免分离元数据发生撕裂。
        let dpr = self.present_surface().device_pixel_ratio.max(0.0001);
        // 计算传给 Win32 drawable 查询的逻辑宽度。
        let logical_width = (resize.extent().width as f32 / dpr).round().max(1.0) as i32;
        // 计算传给 Win32 drawable 查询的逻辑高度。
        let logical_height = (resize.extent().height as f32 / dpr).round().max(1.0) as i32;
        // 直接进入 D3D11 surface 的物理重建路径，避免 RHI 反向依赖兼容入口。
        self.resize_surface_extent(native_width, native_height, logical_width, logical_height)?;
        // 发布前统一验证请求 extent 与 generation 后置条件。
        resize.complete(self.token())
    }

    // 读取当前 D3D11 swapchain surface 并规范化为左上原点 0xAARRGGBB 像素。
    fn read_surface_pixels(&mut self, region: RhiScissor) -> Result<RhiSurfaceReadback> {
        // checked shutdown 后不得访问 swapchain backbuffer。
        self.ensure_active()?;
        // 在进入 D3D11 前执行共享范围验证，禁止旧实现静默裁切。
        RhiSurfaceReadback::validate_region(region, self.token().extent)?;
        // 复用 context 私有的 staging texture 实现。
        let pixels = self.read_surface_pixels_result(
            // 传入共享契约已验证的横坐标。
            region.x,
            // 传入共享契约已验证的纵坐标。
            region.y,
            // 传入共享契约已验证的宽度。
            region.width,
            // 传入共享契约已验证的高度。
            region.height,
        )?;
        // D3D11 BGRA8 小端载荷已经对应规范 0xAARRGGBB，集中验证长度后返回。
        RhiSurfaceReadback::try_new(region, self.token().extent, pixels)
    }

    // 只接受通过共享门禁的 Surface 呈现事务。
    fn present(&mut self, transaction: RhiPresentTransaction) -> Result<()> {
        // checked shutdown 后不得提交旧 frame 或触碰 swapchain。
        self.ensure_active()?;
        // 在同一 Surface 边界冻结当前 swapchain token。
        let current_token = self.token();
        // 从实际 swapchain 形态投影本次 damage 的保留能力。
        let present_coherency = self.surface_capabilities().present_coherency;
        // 在触碰 DXGI 前通过共享门禁验证 frame、目标和最新提交关联。
        let present = self.validate_present_impl(
            // 交付不可拆的 FramePlan 呈现事务。
            transaction,
            // 使用当前 swapchain 的代际与 extent。
            current_token,
            // 使用同一 Surface capability 决定 partial 是否可发布。
            present_coherency,
        )?;
        // 重新绑定 swapchain target，恢复兼容 context 的 owner 状态和 RTV 绑定。
        self.bind_swapchain_target()?;
        // 复用现有的 Present 前后 RTV 生命周期和 DXGI 错误映射。
        self.present_result(&present)
    }

    // 探测已经因遮挡进入 idle 的 D3D11 swapchain 是否恢复可呈现。
    fn test_present(&mut self) -> Result<PresentTestResult> {
        // checked shutdown 后不得继续探测原生 swapchain。
        self.ensure_active()?;
        // DXGI_PRESENT_TEST 不提交帧数据，并固定使用同步间隔零。
        // 把 Windows 专属探测严格留在冻结 swapchain adapter 内。
        self.swap_chain.test_present()
    }

    // 安排下一次 surface acquire 的 typed surface-lost 结果。
    #[cfg(feature = "test-harness")]
    fn inject_surface_lost_for_test(&mut self) -> Result<()> {
        // 只有仍存活的 D3D11 owner 才能登记测试 surface 故障。
        self.ensure_active()?;
        // 在 owner 状态有效时发布下一次 surface lost 标志。
        self.rhi_surface_lost_for_test = true;
        // 返回注入成功结果，不触碰真实 Surface 错误映射。
        Ok(())
    }
}
