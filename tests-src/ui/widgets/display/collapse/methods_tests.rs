//! `src/ui/widgets/display/collapse/methods.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Collapse） ——

impl Collapse {
    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    pub(super) fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32) {
        (
            self.visual.geometry.default_width,
            self.visual.geometry.max_intrinsic_width,
            self.visual.geometry.header_height,
            self.visual.typography.header_font_size,
            self.visual.typography.content_font_size,
            self.visual.geometry.content_horizontal_padding,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    pub(super) fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
