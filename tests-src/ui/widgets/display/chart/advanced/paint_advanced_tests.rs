//! `src/ui/widgets/display/chart/advanced/paint_advanced.rs` 的 cfg(test) 完整辅助实现（自源文件移入）；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// —— 自源文件移入的扩展 impl（impl ChartPlaceholder） ——

impl ChartPlaceholder {
    // 测试目标保留组合图表类型观测入口，供图表语义测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn kind_for_test(&self) -> &'static str {
        use super::super::ChartKind;
        match self.kind {
            ChartKind::Generic => "generic",
            ChartKind::Bar => "bar",
            ChartKind::Line => "line",
            ChartKind::Area => "area",
            ChartKind::Scatter => "scatter",
            ChartKind::Radar => "radar",
            ChartKind::Heatmap => "heatmap",
            ChartKind::Funnel => "funnel",
            ChartKind::Waterfall => "waterfall",
            ChartKind::Combo => "combo",
            ChartKind::Treemap => "treemap",
            ChartKind::Gauge => "gauge",
        }
    }

    // 测试目标保留组合图表 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        self.tooltip_text_at(pos, frame)
    }

    // 测试目标保留组合图表交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留组合图表 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }

    // 测试目标保留组合图表图例布局观测入口，供布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn legend_layout_for_test(&self, frame: Rect) -> Option<(Rect, Rect)> {
        let mut plot = Rect::new(
            frame.x + self.padding,
            frame.y + self.padding,
            (frame.w - self.padding * 2.0).max(0.0),
            (frame.h - self.padding * 2.0).max(0.0),
        );
        if !self.title.is_empty() {
            plot.y += self.visual.layout.title_height;
            plot.h = (plot.h - self.visual.layout.title_height).max(0.0);
        }
        if !self.subtitle.is_empty() {
            plot.y += self.visual.layout.subtitle_height;
            plot.h = (plot.h - self.visual.layout.subtitle_height).max(0.0);
        }
        let (plot, legend) = self.legend_layout(plot);
        legend.map(|legend| (plot, legend))
    }

    // 测试目标保留组合图表动画进度观测入口，供动画测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn animation_progress_for_test(&self) -> Option<f32> {
        self.animation_player
            .as_ref()
            .map(|player| player.opacity_progress)
    }
}
