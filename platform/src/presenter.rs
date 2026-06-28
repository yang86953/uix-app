// ============================================================================
// platform/presenter.rs — 像素呈现器实现
//
// IPresenter / IGraphicsContext trait 已迁移至 crate::api::traits。
// ============================================================================

use crate::api::traits::IPresenter;
use crate::api::types::Error;

// ════════════════════════════════════════════════════════════════════════════
// NullPresenter — 空操作实现
// ════════════════════════════════════════════════════════════════════════════

/// 空操作呈现器（丢弃所有像素）。
///
/// 用于初始化阶段（窗口尚未创建）或测试场景。
#[derive(Default)]
pub struct NullPresenter;

impl NullPresenter {
    pub fn new() -> Self {
        Self
    }
}

impl IPresenter for NullPresenter {
    fn present(
        &mut self,
        _pixels: &[u32],
        _width: i32,
        _height: i32,
        _dirty_rect: Option<(i32, i32, i32, i32)>,
    ) -> std::result::Result<(), Error> {
        Ok(())
    }

    fn resize(&mut self, _width: i32, _height: i32) -> std::result::Result<(), Error> {
        Ok(())
    }
}
