//! `src/draw/raster/pixel_surface.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl PixelSurface） ——

impl PixelSurface {
    /// 当前清除颜色。
    // 测试目标保留清除颜色观测入口，供像素表面契约测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn clear_color(&self) -> Color {
        self.clear_color
    }
}
