//! `src/ui/widgets/display/selectable_list/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl SelectableList） ——

impl SelectableList {
    // 测试目标保留行命中索引观测入口，供列表交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn row_index_at_y(&self, pos_y: f32) -> Option<usize> {
        let frame = self.last_frame.get().unwrap_or_else(|| {
            Rect::new(
                0.0,
                0.0,
                self.visual.geometry.default_width,
                self.visual.geometry.default_height,
            )
        });
        let geometry = self.local_geometry(frame);
        self.row_index_in_geometry(Point::new(geometry.body.x, pos_y), geometry)
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不扩大公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, f32) {
        (
            self.visual.geometry.default_width,
            self.visual.geometry.default_height,
            self.visual.geometry.min_height,
            self.visual.geometry.default_item_height,
            self.visual.geometry.row_gap,
            self.visual.chrome.row_radius,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
