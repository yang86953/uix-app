//! `src/native/backends/windows/tsf_document.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl TsfStoreState） ——

impl TsfStoreState {
    // 测试目标保留 composition 状态观测入口，供 TSF 文档契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn composition_active(&self) -> bool {
        self.composition.active
    }
}
