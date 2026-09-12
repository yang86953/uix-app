//! `src/ui/widgets/display/avatar/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Avatar） ——

impl Avatar {
    // 测试目标保留已加载位图句柄观测入口，供头像资源测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn loaded_handle_for_test(&self) -> Option<BitmapHandle> {
        self.cached.get()
    }
}
