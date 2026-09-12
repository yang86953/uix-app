//! `src/draw/backend/cpu/offscreen.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl CpuOffscreenPool） ——

impl CpuOffscreenPool {
    // 测试目标保留槽位数量观测入口，供离屏池压缩测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn slot_len(&self) -> usize {
        self.pool.len()
    }
}
