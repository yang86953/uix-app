//! PieChart 的 UIX 视觉边界、数据摘要与无分配格式化测试。

use super::*;

#[test]
fn view_build_uses_uix_visual_and_preserves_authored_values() {
    let node = crate::ui::view::View::build(
        PieChart::new()
            .size(224.0)
            .donut(0.35)
            .label_visible(false)
            .label_position(LabelPosition::Outside)
            .legend(LegendPosition::Bottom)
            .padding(9.0),
    );
    let chart = node
        .widget
        .as_any()
        .downcast_ref::<PieChart>()
        .expect("UIX 根必须保留 PieChart Rust 内核");
    let declared = UIX_PIE_CHART_VISUAL
        .get()
        .expect("View 构建必须固化同目录 UIX 视觉");
    assert!(std::ptr::eq(chart.visual, declared));
    assert_eq!(chart.fixed_size, 224.0);
    assert_eq!(chart.hole_radius, 0.35);
    assert!(!chart.label_visible);
    assert_eq!(chart.label_position, LabelPosition::Outside);
    assert_eq!(chart.legend, LegendPosition::Bottom);
    assert_eq!(chart.padding, 9.0);
}

#[test]
fn value_label_formats_without_heap_string() {
    let integer = PieChart::format_value(12.0);
    let decimal = PieChart::format_value(-3.25);
    let huge = PieChart::format_value(f64::MAX);
    assert_eq!(integer.as_str(), "12");
    assert_eq!(decimal.as_str(), "-3.2");
    assert_eq!(huge.as_str(), "1.797693e308");
    // 固定缓冲与单字节长度构成完整值对象，不保存 String 指针。
    assert_eq!(std::mem::size_of_val(&integer), 49);
}

#[test]
fn data_summary_filters_invalid_values_and_preserves_fractions() {
    let chart = PieChart::new().data(vec![
        PieData::new("甲", 1.0, Color::BLUE),
        PieData::new("零", 0.0, Color::GREEN),
        PieData::new("非法", f32::NAN, Color::RED),
        PieData::new("乙", 3.0, Color::GREEN),
    ]);
    let summary = chart.data_summary().expect("正值数据必须形成摘要");
    assert_eq!(summary.count, 2);
    assert_eq!(summary.total, 4.0);
    assert_eq!(summary.max_value, 3.0);
    assert!(summary.has_label);
    let slices = chart.slices_for_test();
    assert_eq!(slices.len(), 2);
    assert_eq!(slices[0], ("甲".to_owned(), 0.25));
    assert_eq!(slices[1], ("乙".to_owned(), 0.75));
}

#[test]
fn duplicate_labels_keep_the_hit_slice_value() {
    let chart = PieChart::new()
        .data(vec![
            PieData::new("同名", 1.0, Color::BLUE),
            PieData::new("同名", 3.0, Color::GREEN),
        ])
        .legend(LegendPosition::None);
    let datum = chart
        .tooltip_datum_at(Point::new(50.0, 100.0), Rect::new(0.0, 0.0, 200.0, 200.0))
        .expect("左侧点必须命中第二个扇区");
    assert_eq!(datum.value, Some(3.0));
    assert_eq!(datum.percentage, Some(0.75));
}
