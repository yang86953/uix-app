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

// 封闭描述一次 draw 恰好选择的顶点或索引范围。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DrawRange {
    // 描述不依赖索引资源的连续顶点范围。
    Vertices {
        // 保存实际提交的顶点数量。
        count: u32,
        // 保存首个顶点位置。
        first: u32,
    },
    // 描述绑定格式后按元素读取的索引范围。
    Indices {
        // 保存不可拆分的索引资源与格式。
        binding: IndexBufferBinding,
        // 保存实际提交的索引数量。
        count: u32,
        // 保存首个索引位置。
        first: u32,
    },
}

// 为两个 Adapter 提供不暴露矛盾组合的范围构造与投影。
impl DrawRange {
    // 把非零元素数量收敛到 OpenGL GLsizei 与 D3D11 UINT 的共同值域。
    const fn count_i32(count: u32) -> Option<i32> {
        // 零数量不可执行，超过 i32 上限会在 OpenGL 转换时改变含义。
        if count == 0 || count > i32::MAX as u32 {
            // 使用空值表达共享原生 ABI 无法表示。
            return None;
        }
        // 已验证值允许无损转换为 OpenGL 有符号参数。
        Some(count as i32)
    }

    // 把顶点起点收敛到 OpenGL GLint 与 D3D11 UINT 的共同值域。
    const fn vertex_first_i32(first: u32) -> Option<i32> {
        // 超过 i32 上限时禁止截断符号位。
        if first > i32::MAX as u32 {
            // 使用空值表达两个 Adapter 的共同子集之外。
            return None;
        }
        // 已验证值允许无损转换。
        Some(first as i32)
    }

    // 创建从第零个顶点开始的非索引范围。
    pub(crate) const fn vertices(count: u32) -> Self {
        // 非索引构造器不接受任何索引字段。
        Self::Vertices { count, first: 0 }
    }

    // 创建一个已经绑定索引资源与格式的索引范围。
    pub(crate) const fn indices(binding: IndexBufferBinding, count: u32, first: u32) -> Self {
        // 共同契约不暴露 OpenGL ES 不支持的 base vertex。
        Self::Indices {
            binding,
            count,
            first,
        }
    }

    // 判断当前范围是否完整落在两个原生 draw ABI 的共同值域。
    pub(crate) const fn is_valid(self) -> bool {
        // 两个变体分别验证自己的数量和起点编码。
        match self {
            // 非索引范围的 count 与 first 都必须可无损转为有符号参数。
            Self::Vertices { count, first } => {
                Self::count_i32(count).is_some() && Self::vertex_first_i32(first).is_some()
            }
            // 索引 count 使用 GLsizei，first 还必须形成有效字节偏移。
            Self::Indices {
                binding,
                count,
                first,
            } => Self::count_i32(count).is_some() && binding.format().byte_offset(first).is_some(),
        }
    }

    // 返回索引范围绑定；非索引范围显式返回空。
    pub(crate) const fn index_binding(self) -> Option<IndexBufferBinding> {
        // 只从 Indices 变体投影索引资源。
        match self {
            // 顶点范围不拥有索引资源。
            Self::Vertices { .. } => None,
            // 索引范围原子返回资源与格式。
            Self::Indices { binding, .. } => Some(binding),
        }
    }

    // 返回供非索引原生命令使用的顶点数量。
    pub(crate) const fn vertex_count(self) -> u32 {
        // 索引范围不得伪造独立顶点 count。
        match self {
            // 顶点变体返回自身数量。
            Self::Vertices { count, .. } => count,
            // 索引变体不使用 Draw 的顶点数量。
            Self::Indices { .. } => 0,
        }
    }

    // 返回 OpenGL 可无损接收的非索引顶点数量。
    pub(crate) const fn vertex_count_i32(self) -> Option<i32> {
        // 只从顶点变体执行共同值域转换。
        match self {
            // 顶点数量必须为正且不超过 GLsizei 上限。
            Self::Vertices { count, .. } => Self::count_i32(count),
            // 索引变体不使用非索引数量。
            Self::Indices { .. } => None,
        }
    }

    // 返回供索引原生命令使用的索引数量。
    pub(crate) const fn index_count(self) -> u32 {
        // 非索引范围不得伪造索引 count。
        match self {
            // 顶点变体不使用 DrawIndexed 的索引数量。
            Self::Vertices { .. } => 0,
            // 索引变体返回自身数量。
            Self::Indices { count, .. } => count,
        }
    }

    // 返回 OpenGL 可无损接收的索引元素数量。
    pub(crate) const fn index_count_i32(self) -> Option<i32> {
        // 只从索引变体执行共同值域转换。
        match self {
            // 非索引变体不使用索引数量。
            Self::Vertices { .. } => None,
            // 索引数量必须为正且不超过 GLsizei 上限。
            Self::Indices { count, .. } => Self::count_i32(count),
        }
    }

    // 返回非索引范围的首顶点位置。
    pub(crate) const fn first_vertex(self) -> u32 {
        // 索引范围不会携带无效的首顶点字段。
        match self {
            // 顶点变体返回自身起点。
            Self::Vertices { first, .. } => first,
            // 索引变体不使用首顶点。
            Self::Indices { .. } => 0,
        }
    }

    // 返回 OpenGL 可无损接收的非索引首顶点位置。
    pub(crate) const fn first_vertex_i32(self) -> Option<i32> {
        // 只从顶点变体执行共同值域转换。
        match self {
            // 首顶点必须不超过 GLint 上限。
            Self::Vertices { first, .. } => Self::vertex_first_i32(first),
            // 索引变体不使用首顶点。
            Self::Indices { .. } => None,
        }
    }

    // 返回索引范围的首索引位置。
    pub(crate) const fn first_index(self) -> u32 {
        // 非索引范围不会携带无效的首索引字段。
        match self {
            // 顶点变体不使用首索引。
            Self::Vertices { .. } => 0,
            // 索引变体返回自身起点。
            Self::Indices { first, .. } => first,
        }
    }
}

// 描述通用 renderer 已经选定的 draw packet。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawPacket {
    // 保存不可拆分的 pipeline 句柄与共享语义。
    pub(crate) pipeline: PipelineBinding,
    // 保存顶点 buffer 句柄。
    pub(crate) vertex_buffer: BufferHandle,
    // 保存可选的 pipeline uniform buffer 句柄。
    pub(crate) uniform_buffer: Option<BufferHandle>,
    // 保存不可表达矛盾字段组合的封闭绘制范围。
    pub(crate) range: DrawRange,
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
            // 默认不使用 Uniform buffer。
            uniform_buffer: None,
            // 使用只能表示非索引范围的封闭变体。
            range: DrawRange::vertices(vertex_count),
        }
    }

    // 判断 packet 是否包含两个 Adapter 都能执行的绘制范围。
    pub(crate) const fn has_valid_range(self) -> bool {
        // 委托给当前唯一范围变体的共同值域门禁。
        self.range.is_valid()
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
        // 构造稳定的 uint32 索引绑定。
        let binding = IndexBufferBinding::new(BufferHandle::from_raw(9), IndexFormat::Uint32);
        // 超大首索引会形成无法由 OpenGL 指针偏移表示的字节位置。
        let overflow_index = DrawRange::indices(binding, 1, i32::MAX as u32);
        // D3D11 也必须服从同一个索引偏移共同子集。
        assert!(!overflow_index.is_valid());
    }
}
