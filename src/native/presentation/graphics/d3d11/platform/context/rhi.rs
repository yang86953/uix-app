//! D3D11 surface 对薄 RHI 的迁移期实现。
//!
//! 本文件只接入 surface 生命周期；低层 device/pass/draw 由同级 RHI device
//! 模块实现，尚未迁移的 UI 语义仍由兼容 `IGraphicsContext` 提供。

// 引入统一错误和结果类型。
use crate::core::error::{Errc, Error, Result};
// 引入最终提交 damage 类型。
use crate::core::PresentDamage;
// 引入 context 上已有的 DPR 快照与无帧探测结果。
use crate::native::present::{IGraphicsContext, PresentTestResult};
// 引入薄 RHI 的 surface 原语。
use crate::native::present::rhi::{
    GraphicsSurface, RenderTargetHandle, RhiExtent, SurfaceFrame, SurfaceToken,
};

// 为 D3D11 context 实现 surface acquire/resize/present。
impl GraphicsSurface for super::D3d11Context {
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
        Ok(SurfaceFrame::new(
            self.token(),
            RenderTargetHandle::from_raw(super::RHI_SURFACE_TARGET_RAW),
        ))
    }

    // 按物理 extent 重建 swapchain，并把输入转换回窗口逻辑尺寸。
    fn resize(&mut self, extent: RhiExtent) -> Result<SurfaceToken> {
        // 拒绝零尺寸，避免把无效 surface 交给 DXGI。
        if !extent.is_positive() {
            // 返回稳定的参数错误。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI surface extent must be positive",
            ));
        }
        // 已经是目标代际和尺寸时不重复重建 swapchain。
        if self.width as u32 == extent.width && self.height as u32 == extent.height {
            // 返回当前 surface token。
            return Ok(self.token());
        }
        // 使用当前 DPR 把物理尺寸转换为兼容 context 的逻辑尺寸。
        // 从单一 surface 快照读取 DPR，避免分离元数据发生撕裂。
        let dpr = self.present_surface().device_pixel_ratio.max(0.0001);
        // 计算传给 Win32 drawable 查询的逻辑宽度。
        let logical_width = (extent.width as f32 / dpr).round().max(1.0) as i32;
        // 计算传给 Win32 drawable 查询的逻辑高度。
        let logical_height = (extent.height as f32 / dpr).round().max(1.0) as i32;
        // 直接进入 D3D11 surface 的物理重建路径，避免 RHI 反向依赖兼容入口。
        self.resize_surface_extent(
            extent.width as i32,
            extent.height as i32,
            logical_width,
            logical_height,
        )?;
        // 返回 ResizeBuffers 成功后推进的 surface token。
        Ok(self.token())
    }

    // 读取当前 D3D11 swapchain surface 的 BGRA 像素。
    fn read_surface_pixels(&mut self, x: i32, y: i32, width: i32, height: i32) -> Result<Vec<u32>> {
        // 复用 context 私有的 staging texture 实现。
        self.read_surface_pixels_result(x, y, width, height)
    }

    // 只接受当前代际的 surface frame，并把提交交给 D3D11 Present。
    fn present(
        &mut self,
        frame: SurfaceFrame,
        _submission: crate::native::present::rhi::SubmissionHandle,
        _damage: PresentDamage,
    ) -> Result<()> {
        // 拒绝旧代际 frame，避免旧 swapchain 的提交伪装成成功。
        if frame.token != self.token() {
            // 返回 surface lost，让上层保留 dirty 并进入恢复 FSM。
            return Err(Error::new(
                Errc::GraphicsSurfaceLost,
                "D3d11 RHI surface frame generation is stale",
            ));
        }
        // 拒绝不是当前 swapchain 的 render target。
        if frame.target.raw() != super::RHI_SURFACE_TARGET_RAW {
            // 返回参数错误，阻止离屏 target 误走最终 present。
            return Err(Error::new(
                Errc::InvalidArgument,
                "D3d11 RHI present requires the acquired surface target",
            ));
        }
        // 重新绑定 swapchain target，恢复兼容 context 的 owner 状态和 RTV 绑定。
        self.bind_swapchain_target()?;
        // 复用现有的 Present 前后 RTV 生命周期和 DXGI 错误映射。
        self.present_result()
    }

    // 探测已经因遮挡进入 idle 的 D3D11 swapchain 是否恢复可呈现。
    fn test_present(&mut self) -> Result<PresentTestResult> {
        // DXGI_PRESENT_TEST 不提交帧数据，并固定使用同步间隔零。
        super::map_dxgi_present_test_result(unsafe {
            // 把 Windows 专属探测严格留在 D3D11 surface adapter 内。
            self.swap_chain.Present(0, super::DXGI_PRESENT_TEST)
        })
    }

    // 安排下一次 surface acquire 的 typed surface-lost 结果。
    #[cfg(feature = "test-harness")]
    fn inject_surface_lost_for_test(&mut self) -> Result<()> {
        self.rhi_surface_lost_for_test = true;
        Ok(())
    }
}
