//! `src/ui/widgets/display/qrcode/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl QRCode） ——

impl QRCode {
    // 测试目标保留二维码模块观测入口，供编码矩阵测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn module(&self, x: usize, y: usize) -> Option<bool> {
        (x < self.module_count && y < self.module_count)
            .then(|| self.modules[y * self.module_count + x])
    }
}
