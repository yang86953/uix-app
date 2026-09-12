//! `src/platform/presentation/contracts/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl GraphicsContextCaps） ——

impl GraphicsContextCaps {
    /// Legal combo: [`RasterMode::Cpu`] × [`PresentMode::PixelUpload`].
    // CPU PixelUpload recipe 目前没有生产 registry 消费者，只保留内部测试构造入口。
    #[cfg(test)]
    pub(crate) fn cpu_pixel_upload(backend: GraphicsApi) -> Self {
        Self {
            backend,
            raster: RasterMode::Cpu,
            present: PresentMode::PixelUpload,
            present_coherency: PresentCoherency::FullOnly,
        }
    }
}
