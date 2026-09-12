//! `src/ui/widgets/display/chart/line_chart/mod.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl LineChart） ——

impl LineChart {
    // 测试目标保留折线几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn geometry_for_test(&self, frame: Rect) -> Option<(f32, Vec<Point>)> {
        self.plot_geometry(frame).map(|plot| {
            let mut points = Vec::new();
            self.fill_points_for_data(&self.data, &plot, &mut points);
            (plot.baseline, points)
        })
    }

    // 测试目标保留折线序列点观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn series_points_for_test(&self, frame: Rect) -> Option<Vec<Vec<Point>>> {
        self.plot_geometry(frame).map(|plot| {
            (0..self.series_count())
                .filter_map(|index| self.series_at(index))
                .map(|data| {
                    let mut points = Vec::new();
                    self.fill_points_for_data(data, &plot, &mut points);
                    points
                })
                .collect()
        })
    }

    // 测试目标保留折线 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    // 测试目标保留折线交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留折线 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }
}
