// 引入当前模块私有值对象。
use super::*;
// 引入测试 packet 使用的共享 pipeline kind。
use crate::platform::presentation::rhi::{PipelineKind, RhiViewport};

// 构造使用固定测试资源身份和调用方范围的完整 packet。
fn packet(range: DrawRange) -> DrawPacket {
    // 一次建立不可拆的 pipeline、Buffer 角色与范围事实。
    DrawPacket::new(
        // 使用测试专用的 SolidMesh pipeline 身份。
        PipelineBinding::for_test(
            // 使用稳定非零 pipeline 句柄。
            crate::platform::presentation::rhi::PipelineHandle::from_raw(1),
            // 选择具有八字节顶点和三十二字节 Uniform 的契约。
            PipelineKind::SolidMesh,
        ),
        // 原子绑定稳定顶点与 Uniform 资源身份。
        DrawBufferBindings::new(BufferHandle::from_raw(2), BufferHandle::from_raw(3)),
        // SolidMesh 不读取纹理，必须显式选择无采样角色。
        DrawSamplingBinding::none(),
        // 当前测试 Draw 显式拥有完整目标 viewport 且不裁剪。
        DrawRasterState::new(
            // 使用稳定正整像素范围。
            RhiViewport {
                // 固定测试宽度。
                width: 20.0,
                // 固定测试高度。
                height: 10.0,
            },
            // 明确关闭额外裁剪。
            None,
        ),
        // 保留调用方指定的互斥绘制范围。
        range,
    )
}

// 锁定 uint32 索引格式的步长、绑定与 checked 偏移。
#[test]
fn uint32_index_binding_owns_stride_and_offset_contract() {
    // 当前封闭索引格式必须固定为四字节。
    assert_eq!(IndexFormat::Uint32.stride_bytes(), 4);
    // 普通首索引必须转换为精确字节偏移。
    assert_eq!(IndexFormat::Uint32.byte_offset(3), Some(12));
    // 超过 OpenGL ES 有符号偏移范围时必须拒绝而不是饱和。
    assert_eq!(IndexFormat::Uint32.byte_offset(u32::MAX), None);
    // 构造不依赖任何原生 API 的索引绑定。
    let binding = IndexBufferBinding::new(BufferHandle::from_raw(7), IndexFormat::Uint32);
    // 绑定必须保留原始资源身份。
    assert_eq!(binding.buffer().raw(), 7);
    // 绑定必须保留元素格式。
    assert_eq!(binding.format(), IndexFormat::Uint32);
    // 非索引范围只能投影顶点字段。
    let vertices = DrawRange::vertices(6);
    // 非索引范围必须保持有效。
    assert!(vertices.is_valid());
    // 非索引范围只暴露顶点数量。
    assert_eq!(vertices.vertex_count(), 6);
    // 非索引范围不得伪造索引绑定或数量。
    assert_eq!(vertices.index_binding(), None);
    // 索引范围必须原子拥有绑定、数量与起点。
    let indices = DrawRange::indices(binding, 3, 2);
    // 索引范围只暴露索引数量。
    assert_eq!(indices.index_count(), 3);
    // 索引范围必须保留首索引位置。
    assert_eq!(indices.first_index(), 2);
    // 索引范围不得伪造非索引顶点数量。
    assert_eq!(indices.vertex_count(), 0);
    // 索引范围必须保留完整绑定。
    assert_eq!(indices.index_binding(), Some(binding));
    // 从零开始的非索引范围末端必须等于数量。
    assert_eq!(vertices.checked_vertex_end(), Some(6));
    // 索引范围不得伪造非索引顶点末端。
    assert_eq!(indices.checked_vertex_end(), None);
}

// 锁定 OpenGL 有符号参数与 D3D11 无符号参数的共同值域。
#[test]
fn draw_range_rejects_native_value_overflow() {
    // 最大有符号元素数量仍属于共同值域。
    let maximum = DrawRange::vertices(i32::MAX as u32);
    // 最大值必须保持有效且可无损投影。
    assert!(maximum.is_valid());
    // OpenGL 投影必须保留原值。
    assert_eq!(maximum.vertex_count_i32(), Some(i32::MAX));
    // 多一个元素会在旧 OpenGL cast 中变成负数。
    let overflow_count = DrawRange::vertices(i32::MAX as u32 + 1);
    // 共享层必须在进入任一 Adapter 前拒绝。
    assert!(!overflow_count.is_valid());
    // 手工构造超出 GLint 的首顶点以覆盖完整值域门禁。
    let overflow_first = DrawRange::Vertices {
        // 保持数量合法以隔离起点失败。
        count: 1,
        // 使用无法无损转成 i32 的起点。
        first: u32::MAX,
    };
    // 首顶点溢出必须由同一门禁拒绝。
    assert!(!overflow_first.is_valid());
    // 构造首顶点与数量相加溢出的非索引范围。
    let overflow_end = DrawRange::Vertices {
        // 保持单个顶点数量本身合法。
        count: 2,
        // 让 checked 末端超过 u32 最大值。
        first: u32::MAX,
    };
    // 末端溢出必须返回空而不是回绕。
    assert_eq!(overflow_end.checked_vertex_end(), None);
    // 构造稳定的 uint32 索引绑定。
    let binding = IndexBufferBinding::new(BufferHandle::from_raw(9), IndexFormat::Uint32);
    // 超大首索引会形成无法由 OpenGL 指针偏移表示的字节位置。
    let overflow_index = DrawRange::indices(binding, 1, i32::MAX as u32);
    // D3D11 也必须服从同一个索引偏移共同子集。
    assert!(!overflow_index.is_valid());
}

// 验证 DrawPacket 共享资源门禁覆盖顶点、Uniform 和索引容量。
#[test]
fn draw_packet_validates_real_buffer_roles_and_capacity() {
    // 构造读取两个 float2 顶点的完整 packet。
    let valid = packet(DrawRange::vertices(2));
    // packet 必须只读暴露创建时冻结的 pipeline 身份。
    assert_eq!(valid.pipeline().kind(), PipelineKind::SolidMesh);
    // packet 必须原子保留顶点与 Uniform 绑定。
    assert_eq!(valid.buffers().vertex().raw(), 2);
    // Uniform 身份不得通过 Option 或字段回填出现。
    assert_eq!(valid.buffers().uniform().raw(), 3);
    // packet 必须只读保留创建时冻结的范围。
    assert_eq!(valid.range(), DrawRange::vertices(2));
    // 合法 vertex/uniform 描述必须通过。
    assert!(
        valid
            .validate_resources(BufferDesc::vertex(16, 8), BufferDesc::uniform(32), None,)
            .is_ok()
    );
    // Uniform 角色错配必须拒绝。
    assert!(
        valid
            .validate_resources(BufferDesc::vertex(16, 8), BufferDesc::vertex(32, 8), None,)
            .is_err()
    );
    // Uniform 容量不匹配必须拒绝。
    assert!(
        valid
            .validate_resources(BufferDesc::vertex(16, 8), BufferDesc::uniform(16), None,)
            .is_err()
    );
    // 顶点资源用途错配必须拒绝。
    assert!(
        valid
            .validate_resources(BufferDesc::uniform(32), BufferDesc::uniform(32), None)
            .is_err()
    );
    // 顶点 stride 错配必须拒绝。
    assert!(
        valid
            .validate_resources(BufferDesc::vertex(16, 16), BufferDesc::uniform(32), None,)
            .is_err()
    );
    // 无效零数量范围必须由 DrawPacket 自身门禁拒绝。
    let empty = packet(DrawRange::vertices(0));
    // 零数量不能越过共享范围门禁。
    assert!(
        empty
            .validate_resources(BufferDesc::vertex(16, 8), BufferDesc::uniform(32), None,)
            .is_err()
    );
    // 非索引范围越过真实顶点容量必须拒绝。
    let vertex_overflow = packet(DrawRange::vertices(3));
    // 三个顶点不能读取只有两个元素的资源。
    assert!(
        vertex_overflow
            .validate_resources(BufferDesc::vertex(16, 8), BufferDesc::uniform(32), None,)
            .is_err()
    );
    // 构造合法索引资源与索引范围。
    let index = BufferHandle::from_raw(4);
    // 一次构造完整索引 packet，不回填任何字段。
    let indexed = packet(DrawRange::indices(
        // 绑定稳定 uint32 索引资源。
        IndexBufferBinding::new(index, IndexFormat::Uint32),
        // 从索引上传中读取两个元素。
        2,
        // 跳过第零个索引元素。
        1,
    ));
    // 索引容量只验证索引读取范围，不推测顶点最大索引。
    assert!(
        indexed
            .validate_resources(
                BufferDesc::vertex(16, 8),
                BufferDesc::uniform(32),
                Some(BufferDesc::index(16, 4)),
            )
            .is_ok()
    );
    // 错误索引资源用途必须拒绝。
    assert!(
        indexed
            .validate_resources(
                BufferDesc::vertex(16, 8),
                BufferDesc::uniform(32),
                Some(BufferDesc::vertex(16, 4)),
            )
            .is_err()
    );
    // 错误索引资源 stride 必须拒绝。
    assert!(
        indexed
            .validate_resources(
                BufferDesc::vertex(16, 8),
                BufferDesc::uniform(32),
                Some(BufferDesc::index(16, 8)),
            )
            .is_err()
    );
    // 索引读取越过真实索引容量必须拒绝。
    assert!(
        indexed
            .validate_resources(
                BufferDesc::vertex(16, 8),
                BufferDesc::uniform(32),
                Some(BufferDesc::index(8, 4)),
            )
            .is_err()
    );
    // 索引首项与数量越过 u32 末端必须拒绝。
    let index_overflow = packet(DrawRange::indices(
        // 复用同一索引资源和格式事实。
        IndexBufferBinding::new(index, IndexFormat::Uint32),
        // 保持单独数量有效。
        2,
        // 让首项与数量发生 checked 溢出。
        u32::MAX,
    ));
    // checked 溢出的索引范围必须在资源验证阶段失败。
    assert!(
        index_overflow
            .validate_resources(
                BufferDesc::vertex(16, 8),
                BufferDesc::uniform(32),
                Some(BufferDesc::index(16, 4)),
            )
            .is_err()
    );
}
