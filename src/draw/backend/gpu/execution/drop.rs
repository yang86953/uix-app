//! [`GpuBackend`] 析构 — backend 子模块。

use crate::draw::backend::contract::RenderBackend;

use super::GpuBackend;

impl Drop for GpuBackend {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            // Drop 边界重试失败经边界观察入口记录。
            crate::diagnostics::observe_boundary_error("gpu/backend_drop", &error);
        }
    }
}
