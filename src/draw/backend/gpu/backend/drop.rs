//! [`GpuBackend`] 析构 — backend 子模块。

use crate::draw::backend::contract::RenderBackend;

use super::GpuBackend;

impl Drop for GpuBackend {
    fn drop(&mut self) {
        if let Err(error) = self.try_shutdown() {
            tracing::error!(
                "GpuBackend: checked shutdown failed: {}",
                error.short_what()
            );
        }
    }
}
