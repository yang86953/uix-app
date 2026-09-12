//! `src/ui/widgets/display/card/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Card） ——

impl Card {
    // 测试目标观察 UIX 声明的关键视觉契约，不暴露到公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
        (
            self.visual.defaults.width,
            self.visual.defaults.height,
            self.visual.defaults.padding,
            self.visual.title.block_height,
            self.visual.title.font_size,
            self.visual.action.height,
            self.visual.action.font_size,
            self.visual.surface.hover_lighten,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }

    // 测试目标保留卡片 frame 注入入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn set_frame_for_test(&self, frame: Rect) {
        self.last_frame.set(Some(Rect::new(
            0.0,
            0.0,
            frame.w.max(0.0),
            frame.h.max(0.0),
        )));
    }
}
