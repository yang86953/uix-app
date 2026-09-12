//! `src/draw/resources/font/service/font_cache.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl GlyphCache） ——

impl GlyphCache {
    // 测试目标保留按条目数配置缓存的构造器，供淘汰策略测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn with_max_entries(max_entries: usize) -> Self {
        Self::with_limits(max_entries, usize::MAX)
    }
}
