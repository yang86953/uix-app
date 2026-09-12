//! `src/draw/backend/contract.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl BackendCapabilities） ——

impl BackendCapabilities {
    // 测试目标保留独立的最小能力配置，供后端契约测试按需构造。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn test() -> Self {
        Self {
            presentation_mode: PresentationMode::ExternalPresenter,
            partial_redraw: true,
            offscreen: false,
            scroll_memmove: false,
        }
    }
}
