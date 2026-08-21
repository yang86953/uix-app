// 引入被测共享值对象和字段偏移。
use super::*;

// 比较规范化坐标，避免十进制表示误差掩盖字段顺序。
fn assert_close(actual: f32, expected: f32) {
    assert!((actual - expected).abs() < 1.0e-6, "{actual} != {expected}");
}

// 非零源域与不同目标原点必须只在共享几何中各解释一次。
#[test]
fn geometry_owns_source_destination_vertices_bounds_and_step() {
    let source_region = RhiTextureRegion::from_xy(10, 20, RhiExtent::new(30, 40));
    let destination_region = RhiTextureRegion::from_xy(100, 120, RhiExtent::new(30, 40));
    let geometry = RhiBlurPassGeometry::new(
        RhiExtent::new(400, 300),
        source_region,
        RhiExtent::new(800, 600),
        destination_region,
    )
    .expect("different origins with equal region extents must be valid");

    assert_eq!(geometry.source_extent(), RhiExtent::new(400, 300));
    assert_eq!(geometry.source_region(), source_region);
    assert_eq!(geometry.target_extent(), RhiExtent::new(800, 600));
    assert_eq!(geometry.destination_region(), destination_region);
    assert_eq!(
        geometry.destination_scissor(),
        RhiScissor {
            x: 100,
            y: 120,
            width: 30,
            height: 40,
        }
    );

    let vertices = geometry.vertex_values();
    // 第一个顶点是目标左下边界与源域左下 UV；两个原点不会相加。
    assert_close(vertices[0], -0.75);
    assert_close(vertices[1], 1.0 - 160.0 / 600.0 * 2.0);
    assert_close(vertices[2], 10.0 / 400.0);
    assert_close(vertices[3], 60.0 / 300.0);
    // 右上顶点同时证明 destination 与 source 可拥有不同原点。
    assert_close(vertices[8], 130.0 / 800.0 * 2.0 - 1.0);
    assert_close(vertices[9], 1.0 - 120.0 / 600.0 * 2.0);
    assert_close(vertices[10], 40.0 / 400.0);
    assert_close(vertices[11], 20.0 / 300.0);

    let mut weights = [0.0f32; BLUR_WEIGHT_COUNT];
    for (index, weight) in weights.iter_mut().enumerate() {
        *weight = (index + 1) as f32 / 100.0;
    }
    let params = geometry.raster_params(RhiBlurDirection::Vertical, 7, &weights);
    let values = params.as_f32s();
    let bounds = &values[BLUR_UV_BOUNDS_FLOAT_OFFSET..BLUR_UV_BOUNDS_FLOAT_OFFSET + 4];
    assert_close(bounds[0], 10.5 / 400.0);
    assert_close(bounds[1], 20.5 / 300.0);
    assert_close(bounds[2], 39.5 / 400.0);
    assert_close(bounds[3], 59.5 / 300.0);
    let step = &values[BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET..BLUR_TEXEL_STEP_TAPS_FLOAT_OFFSET + 4];
    assert_close(step[0], 0.0);
    assert_close(step[1], 1.0 / 300.0);
    assert_eq!(step[2..], [7.0, 0.0]);
    assert_eq!(
        &values[BLUR_WEIGHTS_FLOAT_OFFSET..BLUR_WEIGHTS_FLOAT_OFFSET + BLUR_WEIGHT_COUNT],
        &weights
    );
    assert_eq!(params.encode_ne_bytes().len(), BLUR_UNIFORM_BYTES);
}

// 全目标 Blur 必须保持既有 NDC 与 0..1 UV 画面合同。
#[test]
fn full_target_geometry_preserves_existing_picture_mapping() {
    let extent = RhiExtent::new(8, 4);
    let full = RhiTextureRegion::full(extent);
    let geometry = RhiBlurPassGeometry::new(extent, full, extent, full)
        .expect("full target blur must remain valid");
    let vertices = geometry.vertex_values();
    assert_eq!(
        vertices,
        [
            -1.0, -1.0, 0.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0,
            1.0, 1.0, 1.0, 0.0, -1.0, 1.0, 0.0, 0.0,
        ]
    );
}

// Blur 不允许 Adapter 各自解释缩放或越界区域。
#[test]
fn geometry_gate_rejects_mismatched_or_outside_regions() {
    let extent = RhiExtent::new(16, 12);
    let mismatch = RhiBlurPassGeometry::new(
        extent,
        RhiTextureRegion::from_xy(1, 2, RhiExtent::new(4, 3)),
        extent,
        RhiTextureRegion::from_xy(5, 6, RhiExtent::new(3, 3)),
    )
    .expect_err("blur does not expose scaling semantics");
    assert_eq!(mismatch.code(), Errc::InvalidArgument);

    let outside = RhiBlurPassGeometry::new(
        extent,
        RhiTextureRegion::from_xy(14, 10, RhiExtent::new(4, 3)),
        extent,
        RhiTextureRegion::from_xy(1, 1, RhiExtent::new(4, 3)),
    )
    .expect_err("source region must stay inside sampled texture");
    assert_eq!(outside.code(), Errc::InvalidArgument);
}
