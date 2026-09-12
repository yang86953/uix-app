//! `uix-lang-compiler/src/uix_import.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl SourceStageCache） ——

impl SourceStageCache {
    // 返回当前实际持有的单文件缓存数量，供生命周期契约测试观测。
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.documents.len()
    }
}
