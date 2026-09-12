//! `src/draw/renderer/recovery_driver.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl RebuildRequest） ——

impl RebuildRequest {
    /// Read-only check whether a rebuild is currently requested.
    // 该读取入口只服务同模块单元测试，生产路径通过 take 消费请求。
    #[cfg(test)]
    pub(crate) fn is_requested(&self) -> bool {
        self.requested.load(Ordering::Acquire)
    }
}
