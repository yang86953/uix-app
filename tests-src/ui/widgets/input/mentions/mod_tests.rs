//! `src/ui/widgets/input/mentions/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Mentions） ——

impl Mentions {
    // 测试目标保留 mention 建议状态观测入口，供输入交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn is_suggesting(&self) -> bool {
        self.suggesting
    }

    // 测试目标保留 mention 过滤选项观测入口，供输入交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn filtered_options(&self) -> &[String] {
        &self.filtered
    }
}
