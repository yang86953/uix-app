//! 共享 RHI Buffer 资源表与 Draw 真实资源描述预检。

// 引入 DrawPacket、通用资源表和 Buffer 句柄契约。
use super::{BufferDesc, BufferHandle, DrawPacket, RhiResourceTable};
// 引入共享资源结果类型。
use crate::core::Result;

// 描述资源表必须能够读取的共享 Buffer 事实。
pub(crate) trait RhiBufferResource {
    // 返回资源创建时冻结的 Buffer 描述。
    fn desc(&self) -> BufferDesc;
}

// 保存由真实 Buffer 描述支撑的类型化资源身份。
pub(crate) struct RhiBufferResourceTable<T: RhiBufferResource> {
    // 复用通用资源表的句柄生命周期和陈旧身份门禁。
    entries: RhiResourceTable<BufferHandle, T>,
}

// 为 Buffer 资源提供统一的插入、解析、销毁和 Draw 预检语义。
impl<T: RhiBufferResource> RhiBufferResourceTable<T> {
    // 创建没有任何 Buffer 资源的空表。
    pub(crate) const fn new() -> Self {
        // 委托通用资源表保存句柄槽位状态。
        Self {
            // 初始表不签发任何 Buffer 句柄。
            entries: RhiResourceTable::new(),
        }
    }

    // 登记实际 Buffer 资源并由共享表签发句柄。
    pub(crate) fn insert(&mut self, resource: T) -> BufferHandle {
        // 资源表是 Buffer 句柄的唯一生产 owner。
        self.entries.insert(resource)
    }

    // 读取仍存活的 Buffer 资源。
    pub(crate) fn get(&self, buffer: BufferHandle) -> Result<&T> {
        // 委托通用资源表验证句柄代际。
        self.entries.get(buffer)
    }

    // 可变读取仍存活的 Buffer 资源。
    pub(crate) fn get_mut(&mut self, buffer: BufferHandle) -> Result<&mut T> {
        // 委托通用资源表验证句柄代际。
        self.entries.get_mut(buffer)
    }

    // 检查式取出 Buffer 资源。
    pub(crate) fn take(&mut self, buffer: BufferHandle) -> Result<T> {
        // 委托通用资源表保持销毁后的陈旧身份语义。
        self.entries.take(buffer)
    }

    // 按创建逆序取出所有仍存活的 Buffer 资源。
    pub(crate) fn drain_reverse(&mut self) -> impl Iterator<Item = T> + '_ {
        // 资源 owner 关闭时保持后创建先销毁顺序。
        self.entries.drain_reverse()
    }

    // 解析 DrawPacket 的真实 Buffer 描述并执行共享角色验证。
    pub(crate) fn validate_draw(&self, packet: DrawPacket) -> Result<()> {
        // 先解析 DrawPacket 指定的顶点资源句柄。
        let vertex = self.get(packet.vertex_buffer)?;
        // 复制真实顶点描述，结束资源表借用后继续解析其它角色。
        let vertex_desc = vertex.desc();
        // 解析 DrawPacket 指定的可选 Uniform 资源句柄。
        let uniform_desc = packet
            .uniform_buffer
            // 有 Uniform 身份时必须解析其真实描述。
            .map(|buffer| self.get(buffer).map(|resource| resource.desc()))
            // 把缺失 Uniform 保留为 None 交给 DrawPacket 共享门禁。
            .transpose()?;
        // 解析索引范围实际绑定的可选资源句柄。
        let index_desc = packet
            .range
            .index_binding()
            // 有索引绑定时必须解析其真实描述。
            .map(|binding| self.get(binding.buffer()).map(|resource| resource.desc()))
            // 把缺失索引保留为 None 交给 DrawPacket 共享门禁。
            .transpose()?;
        // 由 DrawPacket 唯一拥有角色、stride、ABI 和容量关系验证。
        packet.validate_resources(vertex_desc, uniform_desc, index_desc)
    }
}

// 验证 Buffer 资源表只把真实描述交给共享 Draw 门禁。
#[cfg(test)]
mod tests {
    // 引入被测资源表和资源 trait。
    use super::{RhiBufferResource, RhiBufferResourceTable};
    // 引入稳定错误分类。
    use crate::core::Errc;
    // 引入 DrawPacket 与 Buffer 共享值对象。
    use crate::native::present::rhi::{
        BufferDesc, BufferHandle, DrawPacket, IndexBufferBinding, IndexFormat, PipelineBinding,
        PipelineHandle, PipelineKind,
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
        // 构造从零开始的两个顶点非索引 draw。
        let mut packet = DrawPacket::triangles(
            // 使用测试专用的共享 SolidMesh pipeline 身份。
            PipelineBinding::for_test(PipelineHandle::from_raw(1), PipelineKind::SolidMesh),
            // 固定两个 float2 顶点。
            2,
        );
        // 绑定真实顶点句柄。
        packet.vertex_buffer = vertex;
        // 绑定真实 Uniform 句柄。
        packet.uniform_buffer = Some(uniform);
        // 返回最小合法 packet。
        packet
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
        // 构造读取三个顶点但只有两个顶点容量的 packet。
        let mut overflow = packet(vertex, uniform);
        // 使用共享非索引范围表达容量越界。
        overflow.range = crate::native::present::rhi::DrawRange::vertices(3);
        // 顶点容量越界必须拒绝。
        assert!(table.validate_draw(overflow).is_err());
        // 登记合法索引资源。
        let index = table.insert(TestBuffer {
            // 四个 uint32 索引共十六字节。
            desc: BufferDesc::index(16, 4),
        });
        // 构造索引读取 packet。
        let mut indexed = packet(vertex, uniform);
        // 绑定两个索引并从第一个索引开始读取。
        indexed.range = crate::native::present::rhi::DrawRange::indices(
            IndexBufferBinding::new(index, IndexFormat::Uint32),
            2,
            1,
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
}
