//! `src/ui/widgets/display/calendar/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl Calendar） ——

impl Calendar {
    // 测试目标保留日历控制区域观测入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn control_rect_for_test(&self) -> Option<Rect> {
        self.interaction_geometry().map(|geometry| geometry.control)
    }

    // 测试目标保留日期中心点观测入口，供交互几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn day_center_for_test(&self, day: usize) -> Option<Point> {
        self.interaction_geometry()?
            .cell_rect(self.year.get(), self.month.get(), day)
            .map(|rect| Point::new(rect.x + rect.w * 0.5, rect.y + rect.h * 0.5))
    }

    // 测试目标观察 UIX 声明的关键视觉契约，不扩大公开 API。
    #[cfg(test)]
    fn visual_contract_for_test(&self) -> (f32, f32, f32, f32, f32, usize) {
        (
            self.visual.geometry.default_cell_size,
            self.visual.geometry.min_cell_size,
            self.visual.geometry.header_height,
            self.visual.geometry.title_height,
            self.visual.chrome.day_radius,
            self.visual.chrome.event_marker_limit,
        )
    }

    // 测试目标确认实例共享同一份 UIX 视觉表。
    #[cfg(test)]
    fn shares_visual_with_for_test(&self, other: &Self) -> bool {
        std::ptr::eq(self.visual, other.visual)
    }
}
