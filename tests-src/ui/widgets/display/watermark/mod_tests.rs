//! `src/ui/widgets/display/watermark/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Watermark） ——

impl Watermark {
    // 测试目标保留水印有效颜色观测入口，供主题样式测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn effective_color_for_test(&self, theme_text: Color) -> Color {
        // 测试入口复用生产颜色解析。
        self.effective_color(theme_text)
    }

    // 测试目标观察 UIX 声明的共享视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, u8) {
        (
            self.visual.tiling.default_opacity,
            self.visual.tiling.default_rotate,
            self.visual.tiling.default_gap_x,
            self.visual.tiling.default_gap_y,
            self.visual.typography.line_height_ratio,
            self.visual.tiling.overscan_tiles,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉配置。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
