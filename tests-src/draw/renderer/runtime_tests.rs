//! `src/draw/renderer/runtime.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Renderer） ——

impl Renderer {
    // 测试目标保留渲染 session 可变观测入口，供 renderer 契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn session_mut(&mut self) -> &mut RenderSession {
        &mut self.session
    }
}
