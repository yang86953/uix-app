//! `src/draw/backend/gpu/execution/impl_main.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl GpuBackend） ——

impl GpuBackend {
    // 测试目标保留 soft upload 字节数观测入口，供 GPU 诊断测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn last_soft_upload_bytes(&self) -> usize {
        self.surface.canvas.last_soft_upload_bytes
    }
}
