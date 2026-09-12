//! `src/ui/widgets/display/chart/bar_chart/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl BarChart） ——

impl BarChart {
    // 测试目标保留柱形几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Rect>)> {
        self.plot_geometry(frame)
            .map(|plot| (plot.baseline, plot.bars))
    }

    // 测试目标保留分组柱形几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn geometry_items_for_test(&self, frame: Rect) -> Option<Vec<(usize, usize, Rect)>> {
        self.plot_geometry(frame).map(|plot| plot.items)
    }

    // 测试目标保留柱形 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    // 测试目标保留柱形交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留柱形 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }
}
