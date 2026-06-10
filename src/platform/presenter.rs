// ============================================================================
// platform/presenter.rs — 像素呈现抽象（Presenter 策略模式）
//
// 设计原则：
//   - 将 "像素上屏" 从 IPlatform 中解耦为独立策略
//   - 每个平台提供自己的 Presenter 实现
//   - 支持运行时热切换（GDI ↔ Direct2D ↔ Vulkan）
//   - NullPresenter 作为安全的默认空实现
// ============================================================================

use crate::diag::Error;

// ════════════════════════════════════════════════════════════════════════════
// IPresenter — 像素呈现接口
// ════════════════════════════════════════════════════════════════════════════

/// Present a pixel buffer to a native window.
///
/// 职责单一：将软件渲染器输出的 `&[u32]` ARGB 像素数据呈现到窗口。
/// 不关心"怎么渲染的"，只关心"怎么上屏的"。
pub trait IPresenter {
    /// Present the pixel buffer to the window.
    ///
    /// `pixels` 是 ARGB 格式的像素数据，长度为 `(width * height)`。
    /// 如果尺寸发生变化，实现方应自动重新创建中间资源。
    fn present(&mut self, pixels: &[u32], width: i32, height: i32) -> Result<(), Error>;

    /// Resize the presentation surface.
    ///
    /// 窗口尺寸变化时调用，释放旧资源并创建适配新尺寸的资源。
    fn resize(&mut self, width: i32, height: i32) -> Result<(), Error>;
}

// ════════════════════════════════════════════════════════════════════════════
// NullPresenter — 空操作实现
// ════════════════════════════════════════════════════════════════════════════

/// A no-op presenter that discards all pixels.
///
/// 用于初始化阶段（窗口尚未创建）或测试场景。
pub struct NullPresenter;

impl NullPresenter {
    pub fn new() -> Self {
        Self
    }
}

impl Default for NullPresenter {
    fn default() -> Self {
        Self::new()
    }
}

impl IPresenter for NullPresenter {
    fn present(&mut self, _pixels: &[u32], _width: i32, _height: i32) -> Result<(), Error> {
        // 空操作：丢弃像素
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }
}
