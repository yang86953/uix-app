// 引入当前模块的 MSDF helpers。
use super::*;

// 验证一个闭合矩形可以生成完整 RGBA8 MSDF 并解码为 coverage。
#[test]
fn msdf_rectangle_payload_and_coverage_are_valid() {
    // 使用顺时针像素空间矩形作为最小闭合轮廓。
    let edges = [
        1.0, 1.0, 7.0, 1.0, 7.0, 1.0, 7.0, 7.0, 7.0, 7.0, 1.0, 7.0, 1.0, 7.0, 1.0, 1.0,
    ];
    // 生成与 GPU shader 同布局的 RGBA8 MSDF。
    let Some(msdf) = msdf_from_edges(&edges, 8, 8) else {
        // 最小矩形轮廓必须可生成 MSDF；附带输入边数与目标尺寸便于定位。
        panic!(
            "rectangle MSDF must generate (edges={}, extent=8x8)",
            edges.len()
        );
    };
    // 每个源像素必须携带 RGB 距离和 A 通道。
    assert_eq!(msdf.len(), 8 * 8 * 4);
    // 把 MSDF 解码为 soft 验证用 R8 coverage。
    let Some(coverage) = coverage_from_msdf(&msdf, 8, 8) else {
        // 有效 MSDF 必须可解码；附带载荷长度便于定位。
        panic!("MSDF must decode (payload={} bytes)", msdf.len());
    };
    // 解码结果必须覆盖整个源 extent。
    assert_eq!(coverage.len(), 8 * 8);
    // 矩形内部应有高 coverage 像素。
    assert!(coverage.iter().any(|value| *value > 200));
    // fringe 或外部应保留低 coverage 像素，证明 AA 不是全屏填充。
    assert!(coverage.iter().any(|value| *value < 50));
}

// 验证 CPU median coverage 与 shader 的距离范围方向一致。
#[test]
fn msdf_encoded_coverage_uses_inside_below_half_convention() {
    // encoded=0.5 表示轮廓边界，应得到半 coverage。
    assert!((msdf_encoded_to_coverage(0.5, 0.5, 0.5) - 0.5).abs() < 1e-6);
    // encoded<0.5 表示内部，应得到满 coverage。
    assert_eq!(msdf_encoded_to_coverage(0.25, 0.25, 0.25), 1.0);
    // encoded>0.5 表示外部，应得到零 coverage。
    assert_eq!(msdf_encoded_to_coverage(0.75, 0.75, 0.75), 0.0);
}
