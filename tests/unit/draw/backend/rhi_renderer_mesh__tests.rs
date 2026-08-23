// 引入被测私有组件。
use super::*;

// 两三角矩形只有四条外边，内部对角线不得生成 coverage 边带。
#[test]
fn rectangle_adds_only_outer_coverage_fringe() {
    let vertices = [
        0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 0.0, 10.0, 10.0, 0.0, 10.0,
    ];
    let antialiased = antialiased_vertices(&vertices);

    // 内部六顶点加四条边各六顶点，共三十个 xyc 顶点。
    assert_eq!(antialiased.len(), 30 * 3);
    let coverage: Vec<_> = antialiased
        .chunks_exact(3)
        .map(|vertex| vertex[2])
        .collect();
    assert_eq!(coverage.iter().filter(|value| **value == 0.0).count(), 12);
    assert_eq!(coverage.iter().filter(|value| **value == 1.0).count(), 18);
}

// 数学边界必须位于 coverage 线性边带中央，保证物理像素覆盖率不偏移。
#[test]
fn rectangle_fringe_is_centered_on_original_boundary() {
    let vertices = [
        0.0, 0.0, 10.0, 0.0, 10.0, 10.0, 0.0, 0.0, 10.0, 10.0, 0.0, 10.0,
    ];
    let antialiased = antialiased_vertices(&vertices);
    let points: Vec<_> = antialiased
        .chunks_exact(3)
        .map(|vertex| ([vertex[0], vertex[1]], vertex[2]))
        .collect();

    let contains = |expected: [f32; 2], coverage: f32| {
        points.iter().any(|(point, actual_coverage)| {
            (point[0] - expected[0]).abs() < 1e-5
                && (point[1] - expected[1]).abs() < 1e-5
                && (*actual_coverage - coverage).abs() < 1e-5
        })
    };
    assert!(contains([-0.5, -0.5], 0.0));
    assert!(contains([0.5, 0.5], 1.0));
    assert!(contains([10.5, 10.5], 0.0));
    assert!(contains([9.5, 9.5], 1.0));
}

// 极薄三角形的尖角 miter 必须保持有界，不能扩展成大面积尖刺。
#[test]
fn thin_triangle_keeps_coverage_fringe_bounded() {
    let vertices = [0.0, 0.0, 0.2, 0.0, 0.1, 0.1];
    let antialiased = antialiased_vertices(&vertices);

    assert!(antialiased.iter().all(|value| value.is_finite()));
    assert!(
        antialiased
            .chunks_exact(3)
            .all(|vertex| vertex[2] >= 0.0 && vertex[2] <= 1.0)
    );
    assert!(
        antialiased
            .chunks_exact(3)
            .flat_map(|vertex| &vertex[..2])
            .all(|coordinate| coordinate.abs() <= MAX_MITER + 0.2)
    );
}
