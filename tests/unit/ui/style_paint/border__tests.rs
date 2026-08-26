// 引入被测纯几何辅助。
use super::*;

// 圆角周长必须闭合且全部点有限。
#[test]
fn border_style_rounded_perimeter_is_closed_and_finite() {
    // 构造带八像素圆角的矩形周长。
    let points = rounded_rect_perimeter(
        // 使用非正方形以覆盖独立宽高限制。
        Rect::new(2.0, 3.0, 40.0, 20.0),
        // 使用统一圆角值。
        Some(Radius::uniform(8.0)),
    );
    // 四个圆角采样应产生多于直角矩形的五个点。
    assert!(points.len() > 5);
    // 起点和终点必须闭合到同一坐标。
    assert_eq!(points.first(), points.last());
    // 每个采样点都必须是有限坐标。
    assert!(
        // 遍历完整周长。
        points
            // 借用每个点。
            .iter()
            // 同时验证两个轴。
            .all(|point| point.x.is_finite() && point.y.is_finite())
    );
}

// 虚线开启区间必须跨圆角采样点保持单一折线。
#[test]
fn border_style_pattern_preserves_continuous_phase_across_corners() {
    // 构造含圆角的闭合周长。
    let perimeter = rounded_rect_perimeter(
        // 使用可产生多个虚线周期的矩形。
        Rect::new(0.0, 0.0, 48.0, 24.0),
        // 圆角需要多个采样点。
        Some(Radius::uniform(6.0)),
    );
    // 使用六像素开启与四像素关闭距离切分。
    let strokes = patterned_subpaths(&perimeter, 6.0, 4.0);
    // 周长必须产生多个可见区间。
    assert!(strokes.len() > 4);
    // 至少一个开启区间应跨越采样点而包含三个以上点。
    assert!(strokes.iter().any(|stroke| stroke.len() > 2));
    // 每个开启区间累计长度不得超过声明 on_length 的浮点容差。
    assert!(strokes.iter().all(|stroke| {
        // 累加相邻点距离。
        let length = stroke
            // 遍历折线相邻点。
            .windows(2)
            // 计算每段距离。
            .map(|pair| distance(pair[0], pair[1]))
            // 合并为总长度。
            .sum::<f32>();
        // 尾段可以更短，但不能更长。
        length <= 6.0 + GEOMETRY_EPSILON * 8.0
    }));
}
