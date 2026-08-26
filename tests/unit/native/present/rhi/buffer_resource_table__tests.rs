// 引入被测资源表和资源 trait。
use super::{RhiBufferResource, RhiBufferResourceTable};
// 引入稳定错误分类。
use crate::core::Errc;
// 引入 DrawPacket 与 Buffer 共享值对象。
use crate::platform::presentation::rhi::{
    BufferDesc, BufferHandle, DrawBufferBindings, DrawPacket, DrawRange, DrawRasterState,
    DrawSamplingBinding, IndexBufferBinding, IndexFormat, PipelineBinding, PipelineHandle,
    PipelineKind, RhiBufferUploadPreflight, RhiViewport,
};

// 保存测试资源的共享描述事实。
struct TestBuffer {
    // 保存不可变 Buffer 创建描述。
    desc: BufferDesc,
}

// 让测试资源满足共享 Buffer 资源表契约。
impl RhiBufferResource for TestBuffer {
    // 返回资源创建时冻结的描述。
    fn desc(&self) -> BufferDesc {
        // 只读复制描述值，不暴露资源内部状态。
        self.desc
    }
}

// 创建使用 SolidMesh ABI 的最小 DrawPacket。
fn packet(vertex: BufferHandle, uniform: BufferHandle) -> DrawPacket {
    // 一次构造完整 pipeline、Buffer 角色与两个顶点范围。
    DrawPacket::new(
        // 使用测试专用的共享 SolidMesh pipeline 身份。
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 原子绑定真实顶点与 Uniform 句柄。
        DrawBufferBindings::new(vertex, uniform),
        // SolidMesh 不读取纹理，显式选择无采样角色。
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
        // 固定两个 float2 顶点。
        DrawRange::vertices(2),
    )
}

// 验证资源表覆盖角色、stride、容量、索引和陈旧句柄。
#[test]
fn validates_draw_roles_and_real_capacity() {
    // 创建空的共享 Buffer 资源表。
    let mut table = RhiBufferResourceTable::new();
    // 登记合法顶点资源。
    let vertex = table.insert(TestBuffer {
        // 两个 float2 顶点共十六字节。
        desc: BufferDesc::vertex(16, 8),
    });
    // 登记合法 Uniform 资源。
    let uniform = table.insert(TestBuffer {
        // SolidMesh 常量 ABI 为三十二字节。
        desc: BufferDesc::uniform(32),
    });
    // 合法非索引 draw 必须通过。
    assert!(table.validate_draw(packet(vertex, uniform)).is_ok());
    // 错误 Uniform 用途必须拒绝。
    let wrong_uniform = table.insert(TestBuffer {
        // 使用顶点用途故意伪造 Uniform 角色。
        desc: BufferDesc::vertex(32, 8),
    });
    // 角色错配必须在共享表中失败。
    assert!(table.validate_draw(packet(vertex, wrong_uniform)).is_err());
    // 错误顶点 stride 必须拒绝。
    let wrong_stride = table.insert(TestBuffer {
        // 使用不匹配 SolidMesh 的十六字节 stride。
        desc: BufferDesc::vertex(32, 16),
    });
    // stride 错配必须在共享表中失败。
    assert!(table.validate_draw(packet(wrong_stride, uniform)).is_err());
    // 构造读取三个顶点但只有两个顶点容量的完整 packet。
    let overflow = DrawPacket::new(
        // 使用与有效路径相同的 SolidMesh pipeline。
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 复用两个存活 Buffer 角色。
        DrawBufferBindings::new(vertex, uniform),
        // SolidMesh 不读取纹理，显式选择无采样角色。
        DrawSamplingBinding::none(),
        // 容量越界测试仍提供合法且完整的动态栅格事实。
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
        // 使用共享非索引范围表达容量越界。
        DrawRange::vertices(3),
    );
    // 顶点容量越界必须拒绝。
    assert!(table.validate_draw(overflow).is_err());
    // 登记合法索引资源。
    let index = table.insert(TestBuffer {
        // 四个 uint32 索引共十六字节。
        desc: BufferDesc::index(16, 4),
    });
    // 构造索引读取完整 packet。
    let indexed = DrawPacket::new(
        // 使用与有效路径相同的 SolidMesh pipeline。
        PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
        // 复用两个存活 Buffer 角色。
        DrawBufferBindings::new(vertex, uniform),
        // SolidMesh 不读取纹理，显式选择无采样角色。
        DrawSamplingBinding::none(),
        // 索引读取测试仍提供合法且完整的动态栅格事实。
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
        // 绑定两个索引并从第一个索引开始读取。
        DrawRange::indices(IndexBufferBinding::new(index, IndexFormat::Uint32), 2, 1),
    );
    // 合法索引读取必须通过，不猜测顶点最大索引。
    assert!(table.validate_draw(indexed).is_ok());
    // 陈旧句柄必须在 Draw 角色验证前拒绝。
    let stale = BufferHandle::from_raw(99);
    // 错误分类必须保持共享参数错误。
    assert_eq!(
        table
            .validate_draw(packet(stale, uniform))
            .unwrap_err()
            .code(),
        Errc::InvalidArgument
    );
}

// 验证资源表上传预检覆盖陈旧句柄、容量和错误用途。
#[test]
fn validates_upload_against_real_buffer_description() {
    // 创建空的共享 Buffer 资源表。
    let mut table = RhiBufferResourceTable::new();
    // 登记一个十六字节顶点 Buffer。
    let vertex = table.insert(TestBuffer {
        // 顶点 stride 固定为八字节。
        desc: BufferDesc::vertex(16, 8),
    });
    // 合法的两个顶点上传必须通过。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::vertex(vertex, 16, 8))
            .is_ok()
    );
    // 半个顶点元素必须拒绝。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::vertex(vertex, 4, 8))
            .is_err()
    );
    // 用途和容量都匹配但 stride 错误的顶点上传必须拒绝。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::vertex(vertex, 16, 16))
            .is_err()
    );
    // 超过 Buffer 容量必须拒绝。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::vertex(vertex, 24, 8))
            .is_err()
    );
    // 同尺寸 Uniform 上传也不得写入真实顶点 Buffer。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::uniform(vertex, 16))
            .is_err()
    );
    // 陈旧句柄必须在真实描述解析前拒绝。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::vertex(
                BufferHandle::from_raw(99),
                16,
                8,
            ))
            .is_err()
    );
    // 登记一个真实索引 Buffer，冻结四字节索引元素 ABI。
    let index = table.insert(TestBuffer {
        // 索引资源容量与 stride 均来自真实 Buffer 描述。
        desc: BufferDesc::index(16, 4),
    });
    // 匹配真实索引 stride 的上传必须通过。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::index(index, 16, 4))
            .is_ok()
    );
    // 用途和容量匹配但索引 stride 错误的上传必须拒绝。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::index(index, 16, 2))
            .is_err()
    );
    // 登记一个 Uniform Buffer 以验证完整替换语义。
    let uniform = table.insert(TestBuffer {
        // Uniform 描述固定为三十二字节。
        desc: BufferDesc::uniform(32),
    });
    // 局部 Uniform 上传必须拒绝。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::uniform(uniform, 16))
            .is_err()
    );
    // 完整 Uniform 上传必须通过。
    assert!(
        table
            .validate_upload(RhiBufferUploadPreflight::uniform(uniform, 32))
            .is_ok()
    );
}
