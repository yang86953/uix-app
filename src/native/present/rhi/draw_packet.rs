//! FramePlan 与图形 Adapter 共享的类型化绘制包。

// 引入不透明 buffer 句柄和已经绑定语义的 pipeline 身份。
use super::{BufferHandle, PipelineBinding};

// 定义两个 Adapter 都必须穷尽映射的索引元素格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum IndexFormat {
    // 保存现有 UIX 索引 ABI 的无符号三十二位整数。
    Uint32,
}

// 为索引格式提供共享步长与 OpenGL 字节偏移算法。
impl IndexFormat {
    // 返回一个索引元素占用的固定字节数。
    pub(crate) const fn stride_bytes(self) -> u32 {
        // 穷尽当前封闭格式集合。
        match self {
            // uint32 固定占四字节。
            Self::Uint32 => std::mem::size_of::<u32>() as u32,
        }
    }

    // 把首索引转换为所有 Adapter 都可验证的有符号字节偏移。
    pub(crate) const fn byte_offset(self, first_index: u32) -> Option<i32> {
        // 使用 checked 乘法拒绝索引元素偏移溢出。
        let Some(bytes) = first_index.checked_mul(self.stride_bytes()) else {
            // 溢出不能饱和后继续进入原生 API。
            return None;
        };
        // OpenGL ES draw_elements 的 offset 必须能被 i32 精确表达。
        if bytes > i32::MAX as u32 {
            // 两个 Adapter 只接受共同可表达的偏移范围。
            return None;
        }
        // 已验证范围允许无损转换。
        Some(bytes as i32)
    }
}

// 把索引 buffer 身份与元素格式绑定为 FramePlan 不可拆事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct IndexBufferBinding {
    // 保存索引资源的不透明句柄。
    buffer: BufferHandle,
    // 保存该资源中每个索引的共享解释。
    format: IndexFormat,
}

// 为索引 buffer 绑定提供唯一构造与只读投影。
impl IndexBufferBinding {
    // 创建已经绑定元素格式的索引资源身份。
    pub(crate) const fn new(buffer: BufferHandle, format: IndexFormat) -> Self {
        // 不在值对象层解释原生资源表。
        Self { buffer, format }
    }

    // 返回供 Adapter 资源表解析的不透明句柄。
    pub(crate) const fn buffer(self) -> BufferHandle {
        // 只复制轻量句柄，不暴露内部字段可变性。
        self.buffer
    }

    // 返回两个 Adapter 必须机械映射的元素格式。
    pub(crate) const fn format(self) -> IndexFormat {
        // 只复制封闭格式值。
        self.format
    }
}

// 描述通用 renderer 已经选定的 draw packet。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawPacket {
    // 保存不可拆分的 pipeline 句柄与共享语义。
    pub(crate) pipeline: PipelineBinding,
    // 保存顶点 buffer 句柄。
    pub(crate) vertex_buffer: BufferHandle,
    // 保存可选且已经绑定元素格式的索引 buffer。
    pub(crate) index_buffer: Option<IndexBufferBinding>,
    // 保存可选的 pipeline uniform buffer 句柄。
    pub(crate) uniform_buffer: Option<BufferHandle>,
    // 保存顶点数量。
    pub(crate) vertex_count: u32,
    // 保存索引数量；零表示非索引绘制。
    pub(crate) index_count: u32,
    // 保存首个顶点位置。
    pub(crate) first_vertex: u32,
    // 保存首个索引位置。
    pub(crate) first_index: u32,
    // 保存索引绘制的基顶点。
    pub(crate) base_vertex: i32,
}

// 为 draw packet 提供常用的非索引三角形构造器。
impl DrawPacket {
    // 创建一个使用类型化 pipeline 的非索引 draw packet。
    pub(crate) const fn triangles(pipeline: PipelineBinding, vertex_count: u32) -> Self {
        // 返回从零开始的非索引绘制范围。
        Self {
            // 保留创建时已经冻结的 pipeline 身份。
            pipeline,
            // 测试或调用方可在构造后绑定实际顶点 buffer。
            vertex_buffer: BufferHandle::from_raw(0),
            // 默认不使用索引 buffer。
            index_buffer: None,
            // 默认不使用 Uniform buffer。
            uniform_buffer: None,
            // 保存调用方声明的顶点数量。
            vertex_count,
            // 非索引绘制不包含索引。
            index_count: 0,
            // 默认从首个顶点开始。
            first_vertex: 0,
            // 非索引绘制不使用首个索引。
            first_index: 0,
            // 非索引绘制不使用基顶点偏移。
            base_vertex: 0,
        }
    }

    // 判断 packet 是否包含可执行的顶点或索引范围。
    pub(crate) const fn is_non_empty(self) -> bool {
        // 索引绘制优先检查 index_count，否则检查 vertex_count。
        self.vertex_count != 0 || self.index_count != 0
    }
}

// 验证索引格式、绑定和偏移算法保持同一共享 ABI。
#[cfg(test)]
mod tests {
    // 引入当前模块私有值对象。
    use super::*;

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
    }
}
