// ============================================================================
// platform/presenter.rs — 像素呈现器实现
//
// IPresenter 与类型化 graphics recipe 生命周期位于 crate::native::present 模块。
// ============================================================================

use crate::core::error::Error;
use crate::native::present::IPresenter;
use crate::native::present::PresentDamage;

// ════════════════════════════════════════════════════════════════════════════
// NullPresenter — 空操作实现
// ════════════════════════════════════════════════════════════════════════════

/// 空操作呈现器（丢弃所有像素）。
///
/// 用于初始化阶段（窗口尚未创建）或测试场景。
#[derive(Default)]
pub(crate) struct NullPresenter;

impl NullPresenter {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl IPresenter for NullPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _damage: PresentDamage,
    ) -> std::result::Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> std::result::Result<(), Error> {
        Ok(())
    }
}
