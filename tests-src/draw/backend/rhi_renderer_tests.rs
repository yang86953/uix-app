//! `src/draw/backend/rhi_renderer.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl RhiRenderer） ——

impl RhiRenderer {
    #[cfg(any(test, target_endian = "big"))]
    fn encode_u32s_big_endian(values: &[u32]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(std::mem::size_of_val(values));
        for value in values {
            bytes.extend_from_slice(&value.to_le_bytes());
        }
        bytes
    }
}
