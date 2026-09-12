//! FramePlan 与图形 Adapter 共享的类型化绘制包。

// 引入绘制资源身份、条件采样角色和已经绑定语义的 pipeline 身份。
use super::{
    BufferDesc, BufferHandle, BufferUsage, DrawRasterState, DrawSamplingBinding, PipelineBinding,
};
// 引入共享错误分类和结果类型。
use crate::core::error::{Errc, Error, Result};

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

    // 返回非索引范围首顶点加数量的 checked 末端。
    pub(crate) const fn checked_vertex_end(self) -> Option<u32> {
        // 只有非索引范围拥有可与顶点载荷比较的末端。
        match self {
            // 使用 checked_add 拒绝 u32 末端溢出。
            Self::Vertices { count, first } => first.checked_add(count),
            // 索引范围不伪造独立的顶点末端。
            Self::Indices { .. } => None,
        }
    }

    // 返回索引范围首项加数量的 checked 字节末端。
    pub(crate) const fn checked_index_end_bytes(self) -> Option<u64> {
        // 只有索引范围拥有可与索引资源容量比较的字节末端。
        match self {
            // 索引末端先按元素 checked 相加，再按格式步长 checked 相乘。
            Self::Indices {
                binding,
                count,
                first,
            } => {
                // 显式匹配 checked 结果，保持当前稳定 Rust 的 const 可用性。
                match first.checked_add(count) {
                    // u32 到 u64 的提升不会丢失索引末端或格式步长。
                    Some(end) => Some(
                        // 乘积最大为 u32 范围乘四，完整落在 u64 内。
                        end as u64 * binding.format().stride_bytes() as u64,
                    ),
                    // 元素末端溢出时拒绝生成任何字节范围。
                    None => None,
                }
            }
            // 非索引范围不伪造索引读取末端。
            Self::Vertices { .. } => None,
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

// 原子保存每次固定 pipeline draw 必需的两个 Buffer 角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct DrawBufferBindings {
    // 保存唯一顶点输入资源身份。
    vertex: BufferHandle,
    // 保存唯一完整 Uniform 常量资源身份。
    uniform: BufferHandle,
}

// 为 Draw Buffer 角色提供完整构造与只读投影。
impl DrawBufferBindings {
    // 一次绑定当前所有固定 pipeline 都必需的两个 Buffer 角色。
    pub(crate) const fn new(vertex: BufferHandle, uniform: BufferHandle) -> Self {
        // 两个身份必须共同建立，禁止缺失 Uniform 的中间 packet。
        Self { vertex, uniform }
    }

    // 返回供 FramePlan 与 Adapter 解析的顶点资源身份。
    pub(crate) const fn vertex(self) -> BufferHandle {
        // 复制轻量不透明句柄，不暴露字段改写能力。
        self.vertex
    }

    // 返回供 FramePlan 与 Adapter 解析的 Uniform 资源身份。
    pub(crate) const fn uniform(self) -> BufferHandle {
        // 复制轻量不透明句柄，不暴露字段改写能力。
        self.uniform
    }
}

// 描述通用 renderer 已经完整绑定的 draw packet。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct DrawPacket {
    // 保存不可拆分的 pipeline 句柄与共享语义。
    pipeline: PipelineBinding,
    // 保存不可缺失的顶点与 Uniform 资源角色。
    buffers: DrawBufferBindings,
    // 保存与 pipeline 采样语义一致的条件资源角色。
    sampling: DrawSamplingBinding,
    // 保存当前 Draw 不依赖历史状态的完整动态栅格事实。
    raster: DrawRasterState,
    // 保存不可表达矛盾字段组合的封闭绘制范围。
    range: DrawRange,
}

// 为 draw packet 提供完整构造、只读投影与共享资源门禁。
impl DrawPacket {
    // 一次创建已经绑定 pipeline、全部资源角色与绘制范围的 packet。
    pub(crate) const fn new(
        // 接收不可拆分的 pipeline 身份与共享语义。
        pipeline: PipelineBinding,
        // 接收当前所有固定 pipeline 都必需的 Buffer 角色。
        buffers: DrawBufferBindings,
        // 接收由当前 pipeline 决定的条件采样资源角色。
        sampling: DrawSamplingBinding,
        // 接收当前 Draw 的完整 viewport 与裁剪选择。
        raster: DrawRasterState,
        // 接收互斥的顶点或索引绘制范围。
        range: DrawRange,
    ) -> Self {
        // 五项事实共同建立后才允许进入 FramePlan。
        Self {
            // 保存不可拆的 pipeline 身份。
            pipeline,
            // 保存完整 Buffer 角色绑定。
            buffers,
            // 保存明确无采样或完整采样资源事实。
            sampling,
            // 保存完整且显式的动态栅格状态。
            raster,
            // 保存互斥绘制范围。
            range,
        }
    }

    // 返回当前 packet 已冻结的 pipeline 身份。
    pub(crate) const fn pipeline(self) -> PipelineBinding {
        // 复制轻量绑定，不暴露字段改写能力。
        self.pipeline
    }

    // 返回当前 packet 不可拆的两个 Buffer 角色。
    pub(crate) const fn buffers(self) -> DrawBufferBindings {
        // 复制完整值对象，保持两个身份共同传播。
        self.buffers
    }

    // 返回当前 packet 封闭的条件采样资源角色。
    pub(crate) const fn sampling(self) -> DrawSamplingBinding {
        // 复制完整值对象，不暴露内部 Option 改写能力。
        self.sampling
    }

    // 返回当前 packet 独占的动态栅格状态。
    pub(crate) const fn raster(self) -> DrawRasterState {
        // 复制完整值对象，不暴露 viewport 或 scissor 回填能力。
        self.raster
    }

    // 返回当前 packet 已冻结的互斥绘制范围。
    pub(crate) const fn range(self) -> DrawRange {
        // 复制封闭枚举，不暴露字段改写能力。
        self.range
    }


    // 判断 packet 的条件采样资源是否与 pipeline 契约一致。
    pub(crate) fn has_valid_sampling(self) -> bool {
        // 由 DrawSamplingBinding 唯一解释无采样与有采样组合。
        self.sampling.matches_pipeline(self.pipeline)
    }

    // 判断 packet 的动态栅格状态是否属于共同原生值域。
    pub(crate) fn has_valid_raster(self) -> bool {
        // 由 DrawRasterState 唯一验证 viewport 与显式 scissor。
        self.raster.is_valid()
    }

    // 判断 packet 是否包含两个 Adapter 都能执行的绘制范围。
    pub(crate) const fn has_valid_range(self) -> bool {
        // 委托给当前唯一范围变体的共同值域门禁。
        self.range.is_valid()
    }

    // 验证 DrawPacket 引用的真实 Buffer 描述与共享 pipeline ABI。
    pub(crate) fn validate_resources(
        // 接收真实顶点资源描述。
        self,
        vertex_desc: BufferDesc,
        // 接收必需的真实 Uniform 资源描述。
        uniform_desc: BufferDesc,
        // 接收可选的真实索引资源描述。
        index_desc: Option<BufferDesc>,
    ) -> Result<()> {
        // 直接 Device 调用也必须拒绝缺失、多余或语义错配的采样角色。
        if !self.has_valid_sampling() {
            // 不允许 Adapter 用历史绑定补齐残缺 packet。
            return Err(draw_resource_error(
                "RHI draw sampling binding does not match pipeline contract",
            ));
        }
        // 直接 Device 调用也必须拒绝不可机械编码的动态栅格状态。
        if !self.has_valid_raster() {
            // 禁止 Adapter 各自量化 viewport 或修正 scissor。
            return Err(draw_resource_error("RHI draw raster state is invalid"));
        }
        // 直接 Device 调用也必须先服从共同 DrawRange 值域门禁。
        if !self.has_valid_range() {
            // 统一拒绝零数量、原生溢出和无效索引偏移。
            return Err(draw_resource_error("RHI draw range is invalid"));
        }
        // 顶点描述必须先通过共同资源值域门禁。
        vertex_desc.validate()?;
        // 顶点资源用途必须与 DrawPacket 角色一致。
        if vertex_desc.usage() != BufferUsage::Vertex {
            // 统一拒绝错误用途而不泄漏 Adapter 名称。
            return Err(draw_resource_error(
                "RHI draw vertex buffer usage is invalid",
            ));
        }
        // 顶点 stride 必须与 pipeline 的共享顶点布局一致。
        if vertex_desc.stride_bytes() != self.pipeline.contract().vertex.stride_bytes() {
            // 统一拒绝 Adapter 各自解释 stride。
            return Err(draw_resource_error(
                "RHI draw vertex buffer stride is invalid",
            ));
        }
        // 非索引范围只能读取实际顶点资源容量内的数据。
        if self.range.index_binding().is_none() {
            // checked 末端溢出不能回绕为合法读取。
            let vertex_end = self
                .range
                .checked_vertex_end()
                .ok_or_else(|| draw_resource_error("RHI draw vertex range end is invalid"))?;
            // 顶点容量由真实 Buffer 描述的字节数和 stride 派生。
            let vertex_capacity =
                (vertex_desc.size_bytes() / vertex_desc.stride_bytes() as usize) as u64;
            // 末端不能超过真实顶点元素容量。
            if u64::from(vertex_end) > vertex_capacity {
                // 统一拒绝越过顶点资源尾部的读取。
                return Err(draw_resource_error("RHI draw vertex range exceeds buffer"));
            }
        }
        // Uniform 描述必须通过共同资源值域门禁。
        uniform_desc.validate()?;
        // Uniform 资源用途必须与 DrawPacket 角色一致。
        if uniform_desc.usage() != BufferUsage::Uniform {
            // 统一拒绝错误用途而不泄漏 Adapter 名称。
            return Err(draw_resource_error(
                "RHI draw uniform buffer usage is invalid",
            ));
        }
        // Uniform 容量必须精确匹配 pipeline 的共享常量 ABI。
        if uniform_desc.size_bytes() != self.pipeline.contract().uniform.size_bytes() {
            // 统一拒绝不同 Adapter 的常量截断或补齐。
            return Err(draw_resource_error(
                "RHI draw uniform buffer size is invalid",
            ));
        }
        // 非索引 DrawPacket 不得额外携带索引资源描述。
        if self.range.index_binding().is_none() {
            // 保持 DrawRange 变体与资源组合不可矛盾。
            if index_desc.is_some() {
                // 统一拒绝未被当前 DrawRange 使用的索引身份。
                return Err(draw_resource_error("RHI draw index buffer is unexpected"));
            }
            // 非索引资源验证已经完成。
            return Ok(());
        }
        // 索引范围必须解析其绑定对应的真实资源描述。
        let index_desc =
            index_desc.ok_or_else(|| draw_resource_error("RHI draw index buffer is missing"))?;
        // 索引描述必须通过共同资源值域门禁。
        index_desc.validate()?;
        // 索引资源用途必须与 DrawRange 角色一致。
        if index_desc.usage() != BufferUsage::Index {
            // 统一拒绝错误用途而不泄漏 Adapter 名称。
            return Err(draw_resource_error(
                "RHI draw index buffer usage is invalid",
            ));
        }
        // 索引资源 stride 必须与绑定格式一致。
        let binding = self
            .range
            .index_binding()
            .ok_or_else(|| draw_resource_error("RHI draw index binding is missing"))?;
        if index_desc.stride_bytes() != binding.format().stride_bytes() {
            // 统一拒绝格式和真实资源 stride 漂移。
            return Err(draw_resource_error(
                "RHI draw index buffer stride is invalid",
            ));
        }
        // checked 索引末端只约束索引 buffer 读取容量。
        let index_end = self
            .range
            .checked_index_end_bytes()
            .ok_or_else(|| draw_resource_error("RHI draw index range end is invalid"))?;
        // 索引末端不得超过真实索引资源的字节容量。
        if index_end > index_desc.size_bytes() as u64 {
            // 不从索引值推测或伪造顶点最大索引。
            return Err(draw_resource_error("RHI draw index range exceeds buffer"));
        }
        // 所有真实 Buffer 描述均满足当前 DrawPacket 角色。
        Ok(())
    }
}

// 构造不含 Adapter 名称的共享 Draw 资源错误。
fn draw_resource_error(message: &'static str) -> Error {
    // 所有资源角色和容量违例统一属于参数错误。
    Error::new(Errc::InvalidArgument, message)
}

#[cfg(test)]
#[path = "../../../../tests-src/platform/presentation/rhi/draw_packet_tests.rs"]
mod draw_packet_tests;
