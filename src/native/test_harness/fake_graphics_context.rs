//! Fake 图形上下文 — 空操作实现，记录调用。

// 引入 fake context 显式呈现校验所需的错误类型。
use crate::core::{Errc, Error};
use crate::native::present::{IGraphicsContext, PresentDamage, PresentFrame};
use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct FakeGraphicsContextState {
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub initialized: bool,
    pub present_calls: usize,
    pub last_present_damage: Option<PresentDamage>,
    pub shutdown_called: bool,
}

impl FakeGraphicsContextState {
    fn new(width: i32, height: i32) -> Self {
        Self {
            width: Cell::new(width),
            height: Cell::new(height),
            // fake context 与生产 adapter 一样在构造成功后立即可用。
            initialized: true,
            present_calls: 0,
            last_present_damage: None,
            shutdown_called: false,
        }
    }
}

#[derive(Debug)]
pub struct FakeGraphicsContext {
    pub state: FakeGraphicsContextState,
}

impl FakeGraphicsContext {
    pub fn new() -> Self {
        Self {
            state: FakeGraphicsContextState::new(0, 0),
        }
    }

    pub fn with_size(width: i32, height: i32) -> Self {
        Self {
            state: FakeGraphicsContextState::new(width, height),
        }
    }
}

impl Default for FakeGraphicsContext {
    fn default() -> Self {
        Self::new()
    }
}

impl IGraphicsContext for FakeGraphicsContext {
    fn caps(&self) -> crate::native::present::GraphicsContextCaps {
        crate::native::present::GraphicsContextCaps::gpu_native_swapchain(
            crate::native::present::GraphicsApi::OpenGlEs,
            crate::core::PresentCoherency::FullOnly,
        )
    }

    fn graphics_backend(&self) -> crate::native::present::GraphicsApi {
        crate::native::present::GraphicsApi::OpenGlEs
    }

    fn try_shutdown(&mut self) -> crate::core::Result<()> {
        self.state.shutdown_called = true;
        Ok(())
    }

    // 返回测试 context 的完整 drawable 元数据快照。
    fn present_surface(&self) -> crate::native::present::PresentSurface {
        // fake 使用 identity DPR 和稳定零 generation。
        crate::native::present::PresentSurface::identity(
            // 读取测试记录的 drawable 宽度。
            self.state.width.get(),
            // 读取测试记录的 drawable 高度。
            self.state.height.get(),
            // fake 不模拟 HiDPI。
            1.0,
            // fake 不模拟同尺寸 surface 重建。
            0,
        )
    }

    // 测试 context 显式记录统一 swapchain payload，不提供旧交换旁路。
    fn present(&mut self, frame: &PresentFrame<'_>) -> crate::core::Result<()> {
        // 按测试 context 宣称的 GPU-native recipe 校验 payload。
        match frame {
            // 记录统一 swapchain 提交及其 damage 事实。
            PresentFrame::Swapchain { damage } => {
                // 统计通过唯一 present 边界的调用。
                self.state.present_calls += 1;
                // 保存本次 damage 供恢复与提交测试断言。
                self.state.last_present_damage = Some(damage.clone());
                // fake 不执行原生提交，直接返回成功。
                Ok(())
            }
            // GPU-native fake 不接受 CPU pixel upload payload。
            PresentFrame::PixelBuffer { .. } => Err(Error::new(
                // payload 与 context recipe 不匹配。
                Errc::InvalidArgument,
                // 明确测试失败来源，避免误判为原生 surface 故障。
                "FakeGraphicsContext: PixelBuffer payload is unsupported",
            )),
        }
    }
}
