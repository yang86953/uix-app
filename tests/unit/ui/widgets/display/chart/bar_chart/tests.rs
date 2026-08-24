//! BarChart 的 UIX 视觉边界与无分配格式化测试。

use super::*;

#[test]
fn view_build_uses_uix_visual_and_preserves_authored_values() {
    let node = crate::ui::view::View::build(
        BarChart::new()
            .height(240.0)
            .show_value(false)
            .bar_radius(7.0)
            .bar_gap(0.3)
            .category_gap(0.4)
            .padding(9.0),
    );
    let chart = node
        .widget
        .as_any()
        .downcast_ref::<BarChart>()
        .expect("UIX 根必须保留 BarChart Rust 内核");
    let declared = UIX_BAR_CHART_VISUAL
        .get()
        .expect("View 构建必须固化同目录 UIX 视觉");
    assert!(std::ptr::eq(chart.visual, declared));
    assert_eq!(chart.fixed_height, 240.0);
    assert!(!chart.show_value);
    assert_eq!(chart.bar_radius, 7.0);
    assert_eq!(chart.bar_gap, 0.3);
    assert_eq!(chart.category_gap, 0.4);
    assert_eq!(chart.padding, 9.0);
}

#[test]
fn value_label_formats_without_heap_string() {
    let integer = BarChart::format_value(12.0);
    let decimal = BarChart::format_value(-3.25);
    assert_eq!(integer.as_str(), "12");
    assert_eq!(decimal.as_str(), "-3.2");
    // 固定缓冲与单字节长度构成完整值对象，不保存 String 指针。
    assert_eq!(std::mem::size_of_val(&integer), 49);
}

#[test]
fn grouped_and_stacked_geometry_preserve_every_series_item() {
    let series = || {
        vec![
            ChartSeries::new(
                "甲",
                vec![
                    BarData::new("Q1", 100.0, Color::BLUE),
                    BarData::new("Q2", 60.0, Color::BLUE),
                ],
            ),
            ChartSeries::new(
                "乙",
                vec![
                    BarData::new("Q1", 40.0, Color::GREEN),
                    BarData::new("Q2", 80.0, Color::GREEN),
                ],
            ),
        ]
    };
    let frame = Rect::new(0.0, 0.0, 400.0, 240.0);

    let grouped = BarChart::new().grouped(true).series(series());
    let grouped_items = grouped
        .geometry_items_for_test(frame)
        .expect("分组数据必须生成柱形几何");
    assert_eq!(
        grouped_items
            .iter()
            .map(|(series, item, _)| (*series, *item))
            .collect::<Vec<_>>(),
        vec![(0, 0), (1, 0), (0, 1), (1, 1)]
    );
    assert!(grouped_items[0].2.x < grouped_items[1].2.x);

    let stacked = BarChart::new().stacked(true).series(series());
    let stacked_items = stacked
        .geometry_items_for_test(frame)
        .expect("堆叠数据必须生成柱形几何");
    assert_eq!(stacked_items.len(), 4);
    assert!((stacked_items[0].2.x - stacked_items[1].2.x).abs() <= f32::EPSILON);
    assert!((stacked_items[0].2.w - stacked_items[1].2.w).abs() <= f32::EPSILON);
    assert!(
        (stacked_items[1].2.y + stacked_items[1].2.h - stacked_items[0].2.y).abs() <= f32::EPSILON
    );
}
