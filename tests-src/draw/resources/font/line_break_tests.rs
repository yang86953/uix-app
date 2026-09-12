//! `src/draw/resources/font/line_break.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl LineBreakMap） ——

impl LineBreakMap {
    // 查询边界是否是 UAX 强制断行。
    #[cfg(test)]
    pub(crate) fn mandatory_at(&self, char_index: usize) -> bool {
        // 精确匹配强制断行类型。
        matches!(
            // 读取目标字符边界并复制轻量枚举。
            self.opportunities.get(char_index).copied().flatten(),
            // 只接受 Mandatory。
            Some(BreakOpportunity::Mandatory)
        )
    }
}
