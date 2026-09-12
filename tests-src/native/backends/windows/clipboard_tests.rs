//! `src/native/backends/windows/clipboard.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 测试目标保留全局内存文本往返入口，供 Windows 剪贴板契约测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn global_memory_text_round_trip(text: &str) -> Option<String> {
    let memory = OwnedGlobalMemory::from_text(text)?;
    LockedGlobalMemory::lock(memory.handle()).map(|locked| locked.text())
}