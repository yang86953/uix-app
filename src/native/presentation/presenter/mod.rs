// ============================================================================
// platform/presenter.rs — 像素呈现器实现
//
// IPresenter 位于 platform 中立合同；具体空实现保留在 native。
// ============================================================================

use crate::core::{Error, PresentDamage};
use crate::platform::presentation::IPresenter;

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
