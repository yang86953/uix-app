//! `ui/widgets/display/chart/advanced/paint_series.rs` 的 cfg(test) 单元测试体；模块层级不变，经 #[path] 引用，不进发布包。

use super::*;

// 极值数据的坐标区应按最大气泡半径从四边内缩。
#[test]
fn bubble_plot_inset_keeps_marker_inside_frame() {
    let plot = Rect::new(10.0, 20.0, 100.0, 80.0);
    let visual = super::super::ADVANCED_CHART_VISUAL_REF.layout;
    let fit = bubble_radius_fit_scale(80.0, plot, visual);
    let radius = scatter_marker_radius(80.0 * fit, true, plot, visual);
    assert_eq!(radius, 16.0);
    assert_eq!(
        inset_scatter_plot(plot, radius),
        Rect::new(26.0, 36.0, 68.0, 48.0)
    );
}

// 等比适配不能把不同大小的气泡压成同一半径。
#[test]
fn bubble_fit_preserves_relative_sizes() {
    let plot = Rect::new(0.0, 0.0, 420.0, 240.0);
    let visual = super::super::ADVANCED_CHART_VISUAL_REF.layout;
    let fit = bubble_radius_fit_scale(64.0, plot, visual);
    let large = scatter_marker_radius(64.0 * fit, true, plot, visual);
    let small = scatter_marker_radius(40.0 * fit, true, plot, visual);
    assert_eq!(large, 48.0);
    assert_eq!(small, 30.0);
}

// 坐标轴标题必须从数据区上下边界各取得独立槽位。
#[test]
fn cartesian_axis_titles_reserve_data_space() {
    let outer = Rect::new(10.0, 20.0, 420.0, 200.0);
    assert_eq!(
        cartesian_data_plot(
            outer,
            true,
            true,
            super::super::ADVANCED_CHART_VISUAL_REF
                .layout
                .axis_title_height,
        ),
        Rect::new(10.0, 36.0, 420.0, 168.0)
    );
}
