// 引入当前共享契约。
use super::*;

// 锁定 position float2 的位置、格式、偏移与步长。
#[test]
fn position_layout_has_one_complete_attribute() {
    // 读取 position 布局。
    let layout = PipelineVertexLayout::PositionF32x2;
    // 共享值域必须有效。
    assert!(layout.is_valid());
    // 步长必须由共享布局唯一拥有。
    assert_eq!(layout.stride_bytes(), 8);
    // OpenGL 投影必须无损。
    assert_eq!(layout.stride_bytes_i32(), Some(8));
    // 布局只能暴露一个属性。
    let [position] = layout.attributes() else {
        // 属性数量变化必须显式失败。
        panic!("position layout must contain one attribute");
    };
    // position 使用零号位置。
    assert_eq!(position.location(), 0);
    // position 使用位置语义。
    assert_eq!(position.semantic(), PipelineVertexSemantic::Position);
    // position 使用 float2 格式。
    assert_eq!(position.format(), PipelineVertexFormat::Float32x2);
    // position 从顶点起点开始。
    assert_eq!(position.offset_bytes(), 0);
}

// 锁定实心网格 position 与 coverage 的紧凑共享序列。
#[test]
fn solid_mesh_layout_has_position_and_scalar_coverage() {
    let layout = PipelineVertexLayout::PositionCoverageF32;
    assert!(layout.is_valid());
    assert_eq!(layout.stride_bytes(), 12);
    let attributes = layout.attributes();
    assert_eq!(attributes.len(), 2);
    assert_eq!(attributes[0].semantic(), PipelineVertexSemantic::Position);
    assert_eq!(attributes[0].format(), PipelineVertexFormat::Float32x2);
    assert_eq!(attributes[1].semantic(), PipelineVertexSemantic::Coverage);
    assert_eq!(attributes[1].format(), PipelineVertexFormat::Float32);
    assert_eq!(attributes[1].offset_bytes(), 8);
}

// 锁定 Blur position 与绝对 source UV 的最小共享序列。
#[test]
fn blur_layout_has_position_and_uv_without_adapter_fields() {
    let layout = PipelineVertexLayout::PositionUvF32;
    assert!(layout.is_valid());
    assert_eq!(layout.stride_bytes(), 16);
    let attributes = layout.attributes();
    assert_eq!(attributes.len(), 2);
    assert_eq!(attributes[0].semantic(), PipelineVertexSemantic::Position);
    assert_eq!(attributes[0].offset_bytes(), 0);
    assert_eq!(
        attributes[1].semantic(),
        PipelineVertexSemantic::TextureCoordinate
    );
    assert_eq!(attributes[1].offset_bytes(), 8);
}

// 锁定 position、uv 与 color 的同一跨 Adapter 序列。
#[test]
fn textured_layout_has_one_shared_attribute_sequence() {
    // 读取完整采样布局。
    let layout = PipelineVertexLayout::PositionUvColorF32;
    // 共享值域必须有效。
    assert!(layout.is_valid());
    // 步长必须覆盖全部三个属性。
    assert_eq!(layout.stride_bytes(), 32);
    // 读取固定属性序列。
    let attributes = layout.attributes();
    // 两个 Adapter 必须共同消费三项。
    assert_eq!(attributes.len(), 3);
    // 位置顺序必须固定为零、一、二。
    assert_eq!(
        // 从三项共享属性投影位置。
        [
            // 读取 position 位置。
            attributes[0].location(),
            // 读取 uv 位置。
            attributes[1].location(),
            // 读取 color 位置。
            attributes[2].location(),
        ],
        // 声明唯一预期位置序列。
        [0, 1, 2]
    );
    // 字节偏移必须固定为零、八、十六。
    assert_eq!(
        // 从三项共享属性投影偏移。
        [
            // 读取 position 偏移。
            attributes[0].offset_bytes(),
            // 读取 uv 偏移。
            attributes[1].offset_bytes(),
            // 读取 color 偏移。
            attributes[2].offset_bytes(),
        ],
        // 声明唯一预期偏移序列。
        [0, 8, 16]
    );
    // 语义顺序必须固定为 position、uv、color。
    assert_eq!(
        // 从共享属性投影语义。
        [
            // 读取 position 语义。
            attributes[0].semantic(),
            // 读取 uv 语义。
            attributes[1].semantic(),
            // 读取 color 语义。
            attributes[2].semantic(),
        ],
        // 声明唯一预期序列。
        [
            // 第一项是位置。
            PipelineVertexSemantic::Position,
            // 第二项是纹理坐标。
            PipelineVertexSemantic::TextureCoordinate,
            // 第三项是颜色。
            PipelineVertexSemantic::Color,
        ]
    );
}
