// 导入被测载荷与共享 viewport。
use super::*;
// 导入构造基础 Uniform 所需的共享 viewport。
use crate::platform::presentation::rhi::RhiViewport;

// 索引载荷必须拒绝空值并保持共享格式、数量和字节大小一致。
#[test]
fn index_payload_validates_shape_and_range() {
    // 构造空索引载荷验证非空门禁。
    let empty = FrameIndexPayload::uint32(Vec::<u32>::new());
    // 空载荷必须无效。
    assert!(!empty.is_valid());
    // 无效载荷不得提供索引数量。
    assert_eq!(empty.index_count(), None);
    // 空载荷的范围查询必须返回无结果。
    assert_eq!(empty.max_index_in_range(0, 1), None);
    // 构造完整的 u32 索引序列。
    let payload = FrameIndexPayload::uint32([7_u32, 2, 9, 4]);
    // 非空且数量可表示的载荷必须有效。
    assert!(payload.is_valid());
    // 载荷格式必须由封闭变体固定为 Uint32。
    assert_eq!(payload.format(), IndexFormat::Uint32);
    // 索引数量必须准确投影为 u32。
    assert_eq!(payload.index_count(), Some(4));
    // 字节大小必须等于元素数乘以 u32 宽度。
    assert_eq!(payload.size_bytes(), 4 * std::mem::size_of::<u32>());
    // 合法非空子范围必须返回所选元素最大值。
    assert_eq!(payload.max_index_in_range(1, 2), Some(9));
    // 覆盖完整序列的范围也必须返回最大值。
    assert_eq!(payload.max_index_in_range(0, 4), Some(9));
    // 空范围不得伪造最大索引。
    assert_eq!(payload.max_index_in_range(0, 0), None);
    // 起点落在末端之后的范围必须被拒绝。
    assert_eq!(payload.max_index_in_range(4, 1), None);
    // 末端溢出载荷长度的范围必须被拒绝。
    assert_eq!(payload.max_index_in_range(3, 2), None);
    // 编码长度必须与类型化字节大小一致。
    let encoded = payload.encode_ne_bytes();
    // 编码不得丢失或增加任何索引字节。
    assert_eq!(encoded.len(), payload.size_bytes());
    // 以同一 native-endian 规则构造精确期望字节。
    let mut expected = Vec::new();
    // 按原始索引顺序拼接每个 u32 的 native-endian 字节。
    for value in [7_u32, 2, 9, 4] {
        // 期望表示必须保持紧密无填充布局。
        expected.extend_from_slice(&value.to_ne_bytes());
    }
    // 实际编码内容必须与唯一编码规则完全一致。
    assert_eq!(encoded, expected);
    // 零分配只读视图必须逐字节保持同一 native-endian 编码。
    assert_eq!(payload.as_ne_bytes(), expected);
}

// 顶点载荷必须按声明布局验证完整且有限的顶点。
#[test]
fn vertex_payload_rejects_partial_or_non_finite_vertices() {
    // 构造完整三角形 position-float2 顶点。
    let valid = FrameVertexPayload::position_f32x2([0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
    // 完整有限顶点必须通过。
    assert!(valid.is_valid());
    // 编码长度必须等于六个浮点。
    assert_eq!(valid.size_bytes(), 6 * std::mem::size_of::<f32>());
    // 零分配字节视图必须与既有逐值编码完全一致。
    assert_eq!(valid.as_ne_bytes(), valid.encode_ne_bytes());
    // 残缺 float8 顶点必须被拒绝。
    assert!(!FrameVertexPayload::position_uv_color_f32([0.0; 7]).is_valid());
    // 非有限 position 顶点必须在进入 Adapter 前被拒绝。
    assert!(!FrameVertexPayload::position_f32x2([f32::NAN, 0.0]).is_valid());
    // position-float2 的六个浮点必须派生三个完整顶点。
    assert_eq!(valid.vertex_count(), Some(3));
    // 残缺 float8 载荷不得派生顶点数量。
    assert_eq!(
        FrameVertexPayload::position_uv_color_f32([0.0; 7]).vertex_count(),
        None
    );
}

// float8 顶点载荷必须按完整布局派生顶点数量。
#[test]
fn vertex_payload_derives_float8_vertex_count() {
    // 构造两个完整的 position/uv/color 顶点。
    let payload = FrameVertexPayload::position_uv_color_f32([0.0; 16]);
    // 每八个浮点必须派生一个顶点。
    assert_eq!(payload.vertex_count(), Some(2));
}

// sampled 顶点颜色必须在共享 FramePlan 的单位域内。
#[test]
fn vertex_payload_rejects_non_unit_sampled_colors() {
    // 单位颜色域的两个边界值必须通过。
    assert!(
        FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.5, 1.0,])
            .is_valid()
    );
    // 负颜色通道必须在进入任一 Adapter 前被拒绝。
    assert!(
        !FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, -0.01, 0.5, 0.5, 1.0,])
            .is_valid()
    );
    // 超过单位上界的颜色通道必须被拒绝。
    assert!(
        !FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, 0.5, 1.01, 0.5, 1.0,])
            .is_valid()
    );
    // 构造两个完整顶点，证明门禁会逐顶点检查而不是只看首项。
    let mut second_vertex_invalid = [0.0; 16];
    // 首个顶点使用完整合法颜色。
    second_vertex_invalid[4..8].copy_from_slice(&[0.0, 0.5, 1.0, 1.0]);
    // 第二个顶点在蓝通道越过单位上界。
    second_vertex_invalid[14] = 1.01;
    // 任一后续顶点越界都必须拒绝整个类型化载荷。
    assert!(!FrameVertexPayload::position_uv_color_f32(second_vertex_invalid).is_valid());
    // NaN 颜色必须被通用有限值门禁拒绝。
    assert!(
        !FrameVertexPayload::position_uv_color_f32([0.0, 0.0, 0.0, 0.0, f32::NAN, 0.5, 0.5, 1.0,])
            .is_valid()
    );
    // 无穷颜色必须被通用有限值门禁拒绝。
    assert!(
        !FrameVertexPayload::position_uv_color_f32([
            0.0,
            0.0,
            0.0,
            0.0,
            0.5,
            f32::INFINITY,
            0.5,
            1.0,
        ])
        .is_valid()
    );
}

// Uniform 载荷布局和编码长度必须由同一闭集映射拥有。
#[test]
fn uniform_payload_layout_owns_exact_encoded_size() {
    // 构造可区分的确定 viewport。
    let viewport = RhiViewport {
        // 保存测试宽度。
        width: 320.0,
        // 保存测试高度。
        height: 180.0,
    };
    // 构造 sampled 类型化常量。
    let payload = FrameUniformPayload::Sampled(RhiSampledRasterParams::new(viewport));
    // 闭集布局必须映射到 Sampled。
    assert_eq!(payload.layout(), PipelineUniformLayout::Sampled);
    // 编码长度必须与同一布局大小完全一致。
    assert_eq!(payload.encode_ne_bytes().len(), payload.size_bytes());
    // 固定 Uniform 的借用字节视图必须与既有编码器逐字节一致。
    assert_eq!(payload.as_ne_bytes(), payload.encode_ne_bytes());
    // 构造器生成的确定字段必须全部有限。
    assert!(payload.is_valid());
}
