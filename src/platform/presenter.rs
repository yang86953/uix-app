// ============================================================================
// platform/presenter.rs — NullPresenter（空操作实现）
//
// IPresenter trait 定义已迁至 api.rs。
// 此文件只保留具体实现。
// ============================================================================

use crate::diag::Error;
use crate::platform::api::IPresenter;

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
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> Result<(), Error> {
        Ok(())
    }
}
