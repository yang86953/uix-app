//! 共享 RHI Buffer 资源表与 Draw 真实资源描述预检。

// 引入 DrawPacket、上传预检、通用资源表和 Buffer 句柄契约。
use super::{BufferDesc, BufferHandle, DrawPacket, RhiBufferUploadPreflight, RhiResourceTable};
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
        // 一次取得不可拆的顶点与 Uniform 资源身份。
        let buffers = packet.buffers();
        // 先解析 DrawPacket 指定的顶点资源句柄。
        let vertex = self.get(buffers.vertex())?;
        // 复制真实顶点描述，结束资源表借用后继续解析其它角色。
        let vertex_desc = vertex.desc();
        // 解析 DrawPacket 必需的 Uniform 资源句柄。
        let uniform_desc = self.get(buffers.uniform())?.desc();
        // 解析索引范围实际绑定的可选资源句柄。
        let index_desc = packet
            // 只读取得不可变 DrawRange。
            .range()
            .index_binding()
            // 有索引绑定时必须解析其真实描述。
            .map(|binding| self.get(binding.buffer()).map(|resource| resource.desc()))
            // 把缺失索引保留为 None 交给 DrawPacket 共享门禁。
            .transpose()?;
        // 由 DrawPacket 唯一拥有角色、stride、ABI 和容量关系验证。
        packet.validate_resources(vertex_desc, uniform_desc, index_desc)
    }

    // 解析真实 Buffer 描述并验证一次只读上传预检。
    pub(crate) fn validate_upload(&self, upload: RhiBufferUploadPreflight) -> Result<()> {
        // 先解析真实句柄，陈旧身份不得进入描述验证。
        let resource = self.get(upload.buffer())?;
        // 只把资源创建时冻结的描述交给共享预检值对象。
        upload.validate(resource.desc())
    }
}
