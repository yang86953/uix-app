//! `src/native/backends/windows/text_input.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl WindowsTextInput） ——

impl WindowsTextInput {
    // 测试目标保留 TSF client id 观测入口，供输入会话契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tsf_client_id(&self) -> Option<u32> {
        self.tsf.as_ref().map(TsfSession::client_id)
    }
}
