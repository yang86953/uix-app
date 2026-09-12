//! `src/native/presentation/graphics/vulkan/adapter/context/swapchain.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl PresentCompletion） ——

impl PresentCompletion {
    #[cfg(any(test, uix_gpu_parity_vulkan))]
    pub(crate) fn new(image_count: usize) -> Result<Self> {
        Ok(Self {
            presented_images: allocate_presented_images(image_count)?,
            release_after_submission: 0,
        })
    }
}
