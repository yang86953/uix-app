//! FramePlan 与图形 Adapter 共享的类型化绘制包。

// 引入不透明 buffer 句柄和已经绑定语义的 pipeline 身份。
use super::{BufferHandle, PipelineBinding};

// 描述通用 renderer 已经选定的 draw packet。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DrawPacket {
    // 保存不可拆分的 pipeline 句柄与共享语义。
    pub(crate) pipeline: PipelineBinding,
    // 保存顶点 buffer 句柄。
    pub(crate) vertex_buffer: BufferHandle,
    // 保存可选索引 buffer 句柄。
    pub(crate) index_buffer: Option<BufferHandle>,
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
