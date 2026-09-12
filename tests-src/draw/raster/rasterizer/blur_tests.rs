//! `src/draw/raster/rasterizer/blur.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的自由 cfg(test) 项 ——

// 测试目标保留线程本地 kernel cache 长度观测入口，供模糊缓存测试按需调用。
#[cfg_attr(test, allow(dead_code))]
#[cfg(test)]
pub(crate) fn kernel_cache_len_for_test() -> usize {
    KERNEL_CACHE.with(|cache| cache.borrow().len())
}