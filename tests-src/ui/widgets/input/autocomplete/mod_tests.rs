//! `src/ui/widgets/input/autocomplete/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl AutoComplete） ——

impl AutoComplete {
    // 测试目标保留过滤选项观测入口，供自动完成匹配测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn filtered_options(&self) -> &[String] {
        &self.filtered
    }
}
