//! `src/ui/widgets/feedback/progress/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl ProgressBar） ——

impl ProgressBar {
    // 测试目标只读暴露 UIX 视觉默认值，不把私有配置类型扩展到生产 API。
    #[cfg(test)]
    pub(crate) fn visual_contract_for_test(&self) -> (f32, f32, f32) {
        (
            self.visual.layout.default_width,
            self.visual.layout.default_height,
            self.visual.motion.phase_speed,
        )
    }

    // 测试目标验证实例只共享视觉配置引用，不复制完整参数表。
    #[cfg(test)]
    pub(crate) fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }

    // 测试目标确认构建前后都直接借用 UIX 生成的唯一静态视觉值。
    #[cfg(test)]
    pub(crate) fn uses_declared_visual_for_test(&self) -> bool {
        std::ptr::eq(self.visual, PROGRESS_VISUAL_REF)
    }
}
