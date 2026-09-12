//! `src/native/windowing/shared/ime_owner.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl NativeImeOwner） ——

impl NativeImeOwner {
    #[cfg(test)]
    pub(crate) fn selected(&self) -> Option<NativeImeTarget> {
        self.selected
    }
}
