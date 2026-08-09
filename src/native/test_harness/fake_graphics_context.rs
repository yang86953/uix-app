//! Fake 图形上下文 — 空操作实现，记录调用。

// 引入 fake context 需要实现的最小图形上下文契约。
use crate::native::present::IGraphicsContext;
use std::cell::Cell;

#[derive(Debug, Clone)]
pub struct FakeGraphicsContextState {
    pub width: Cell<i32>,
    pub height: Cell<i32>,
    pub initialized: bool,
    pub shutdown_called: bool,
}

impl FakeGraphicsContextState {
    fn new(width: i32, height: i32) -> Self {
        Self {
            width: Cell::new(width),
            height: Cell::new(height),
            // fake context 与生产 adapter 一样在构造成功后立即可用。
            initialized: true,
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
}
