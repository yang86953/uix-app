//! LineChart 的 UIX 视觉边界与热路径缓冲复用测试。

use super::*;

#[test]
fn view_build_uses_uix_visual_and_preserves_authored_values() {
    let node = crate::ui::view::View::build(
        LineChart::new()
            .height(236.0)
            .show_grid(false)
            .show_dots(false)
            .line_width(4.0)
            .dot_radius(6.0)
            .padding(8.0),
    );
    let chart = node
        .widget
        .as_any()
        .downcast_ref::<LineChart>()
        .expect("UIX 根必须保留 LineChart Rust 内核");
    let declared = UIX_LINE_CHART_VISUAL
        .get()
        .expect("View 构建必须固化同目录 UIX 视觉");
    assert!(std::ptr::eq(chart.visual, declared));
    assert_eq!(chart.fixed_height, 236.0);
    assert!(!chart.show_grid);
    assert!(!chart.show_dots);
    assert_eq!(chart.line_width, 4.0);
    assert_eq!(chart.dot_radius, 6.0);
    assert_eq!(chart.padding, 8.0);
}

#[test]
fn value_label_formats_without_heap_string() {
    let integer = LineChart::format_value(12.0);
    let decimal = LineChart::format_value(-3.25);
    assert_eq!(integer.as_str(), "12");
    assert_eq!(decimal.as_str(), "-3.2");
    // 固定缓冲与单字节长度构成完整值对象，不保存 String 指针。
    assert_eq!(std::mem::size_of_val(&integer), 49);
}

#[test]
fn multi_series_geometry_preserves_every_series_point() {
    let chart = LineChart::new().series(vec![
        ChartSeries::new(
            "甲",
            vec![LineData::new("Q1", 20.0), LineData::new("Q2", 40.0)],
        ),
        ChartSeries::new(
            "乙",
            vec![LineData::new("Q1", 30.0), LineData::new("Q2", 10.0)],
        ),
    ]);
    let series = chart
        .series_points_for_test(Rect::new(0.0, 0.0, 400.0, 240.0))
        .expect("多序列数据必须生成折线几何");
    assert_eq!(series.len(), 2);
    assert_eq!(series[0].len(), 2);
    assert_eq!(series[1].len(), 2);
    assert!((series[0][0].x - series[1][0].x).abs() <= f32::EPSILON);
    assert!(series[0][0].y > series[0][1].y);
    assert!(series[1][0].y < series[1][1].y);
}

#[test]
fn smooth_sampling_reuses_output_without_changing_geometry() {
    let points = [
        Point::new(0.0, 20.0),
        Point::new(10.0, 10.0),
        Point::new(20.0, 30.0),
    ];
    let expected = super::super::advanced::catmull_rom_points(&points, 8);
    let mut output = Vec::new();
    super::super::advanced::catmull_rom_points_into(&points, 8, &mut output);
    assert_eq!(output, expected);

    let capacity = output.capacity();
    super::super::advanced::catmull_rom_points_into(&points, 8, &mut output);
    assert_eq!(output, expected);
    assert_eq!(output.capacity(), capacity);
}

#[test]
fn sync_preserves_allocated_geometry_buffers() {
    let mut chart = LineChart::new();
    chart.points_scratch.get_mut().reserve(64);
    chart.smooth_points_scratch.get_mut().reserve(512);
    let point_capacity = chart.points_scratch.get_mut().capacity();
    let smooth_capacity = chart.smooth_points_scratch.get_mut().capacity();

    chart.sync_from(LineChart::new().smooth(true));

    assert_eq!(chart.points_scratch.get_mut().capacity(), point_capacity);
    assert_eq!(
        chart.smooth_points_scratch.get_mut().capacity(),
        smooth_capacity
    );
}
