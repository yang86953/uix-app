//! `ui/widgets/display/chart/pie_chart/mod.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;
use super::safe_donut_label_radius;

// 环图标签必须完整位于白色内环与扇区外缘之间。
#[test]
fn donut_label_radius_avoids_hole_and_outer_edge() {
    let radius =
        safe_donut_label_radius(40.0, 44.0, 80.0, 10.0, 2.0).expect("当前环宽足以容纳标签");
    assert_eq!(radius, 56.0);
    assert!(radius - 10.0 > 44.0);
    assert!(radius + 10.0 < 80.0);
}

// 环宽不足时不得把标签画进白色内环。
#[test]
fn narrow_donut_skips_unsafe_inside_label() {
    assert_eq!(safe_donut_label_radius(48.0, 44.0, 54.0, 6.0, 2.0), None);
}

// —— 自源文件移入的扩展 impl（impl PieChart） ——

impl PieChart {
    // 测试目标保留饼图切片观测入口，供图表数据测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn slices_for_test(&self) -> Vec<(String, f32)> {
        let Some(summary) = self.data_summary() else {
            return Vec::new();
        };
        self.valid_data()
            .map(|data| (data.label.clone(), summary.fraction(data.value)))
            .collect()
    }

    // 测试目标保留饼图总扫掠角观测入口，供图表几何测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn sweep_degrees_for_test(&self) -> f32 {
        self.sweep_degrees()
    }

    // 测试目标保留饼图扇区几何观测入口，供图表布局测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn sector_geometry_for_test(&self) -> Vec<(f32, f32)> {
        let Some(summary) = self.data_summary() else {
            return Vec::new();
        };
        let max_fraction = summary.max_fraction();
        self.valid_data()
            .map(|data| {
                let fraction = summary.fraction(data.value);
                let angle_fraction = if self.rose {
                    1.0 / summary.count as f32
                } else {
                    fraction
                };
                (
                    angle_fraction * self.sweep_degrees(),
                    self.slice_radius(fraction, max_fraction, 1.0),
                )
            })
            .collect()
    }

    // 测试目标保留饼图 tooltip 文本观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_text_for_test(&self, pos: Point, frame: Rect) -> Option<String> {
        let config = self.tooltip_config.as_ref()?;
        Some(config.format(&self.tooltip_datum_at(pos, frame)?))
    }

    // 测试目标保留饼图交互状态观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn interaction_state_for_test(&self) -> (f32, f32) {
        (self.zoom.get(), self.pan_offset.get())
    }

    // 测试目标保留饼图 tooltip 位置观测入口，供交互测试按需调用。
    #[cfg_attr(test, allow(dead_code))]
    #[cfg(test)]
    pub(crate) fn tooltip_position_for_test(&self) -> Option<Point> {
        self.tooltip_pos.get()
    }
}
