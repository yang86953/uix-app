//! Drawing System 与原生图形 Adapter 共享的顶点输入 ABI。

// 定义全部共享 pipeline 使用的顶点属性槽位数量。
pub(crate) const PIPELINE_VERTEX_ATTRIBUTE_SLOT_COUNT: u32 = 3;

// 定义 shader 与 Adapter 必须一致解释的顶点属性语义。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineVertexSemantic {
    // 表示顶点位置输入。
    Position,
    // 表示纹理坐标输入。
    TextureCoordinate,
    // 表示逐顶点颜色输入。
    Color,
    // 表示实心网格边缘的逐顶点覆盖率输入。
    Coverage,
}

// 定义两个 Adapter 必须穷尽映射的顶点属性格式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineVertexFormat {
    // 表示一个三十二位浮点数。
    Float32,
    // 表示两个连续的三十二位浮点数。
    Float32x2,
    // 表示四个连续的三十二位浮点数。
    Float32x4,
}

// 为共享顶点格式提供机械映射所需的数值事实。
impl PipelineVertexFormat {
    // 返回 OpenGL 属性接口使用的分量数量。
    pub(crate) const fn component_count_i32(self) -> i32 {
        // 穷尽封闭格式集合。
        match self {
            // float1 固定包含一个分量。
            Self::Float32 => 1,
            // float2 固定包含两个分量。
            Self::Float32x2 => 2,
            // float4 固定包含四个分量。
            Self::Float32x4 => 4,
        }
    }

    // 返回一个属性占用的固定字节数。
    pub(crate) const fn size_bytes(self) -> u32 {
        // 穷尽封闭格式集合。
        match self {
            // 一个 f32 固定占四字节。
            Self::Float32 => std::mem::size_of::<f32>() as u32,
            // 两个 f32 固定占八字节。
            Self::Float32x2 => 2 * std::mem::size_of::<f32>() as u32,
            // 四个 f32 固定占十六字节。
            Self::Float32x4 => 4 * std::mem::size_of::<f32>() as u32,
        }
    }
}

// 把一个顶点属性的语义、格式、位置和偏移绑定为不可拆事实。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct PipelineVertexAttribute {
    // 保存 OpenGL shader 使用的稳定位置。
    location: u32,
    // 保存 D3D11 shader 使用的稳定语义。
    semantic: PipelineVertexSemantic,
    // 保存两个 Adapter 必须使用的元素格式。
    format: PipelineVertexFormat,
    // 保存属性相对单个顶点起点的字节偏移。
    offset_bytes: u32,
}

// 为类型化顶点属性提供只读投影。
impl PipelineVertexAttribute {
    // 创建一项完整的共享顶点属性事实。
    const fn new(
        // 接收 shader 输入位置。
        location: u32,
        // 接收 shader 输入语义。
        semantic: PipelineVertexSemantic,
        // 接收元素格式。
        format: PipelineVertexFormat,
        // 接收顶点内字节偏移。
        offset_bytes: u32,
    ) -> Self {
        // 同时保存全部字段，禁止 Adapter 拆分构造。
        Self {
            // 冻结输入位置。
            location,
            // 冻结输入语义。
            semantic,
            // 冻结元素格式。
            format,
            // 冻结顶点内偏移。
            offset_bytes,
        }
    }

    // 返回 OpenGL 使用的稳定属性位置。
    pub(crate) const fn location(self) -> u32 {
        // 位置是轻量复制值。
        self.location
    }

    // 返回 D3D11 使用的稳定属性语义。
    pub(crate) const fn semantic(self) -> PipelineVertexSemantic {
        // 语义是封闭枚举复制值。
        self.semantic
    }

    // 返回两个 Adapter 必须映射的属性格式。
    pub(crate) const fn format(self) -> PipelineVertexFormat {
        // 格式是封闭枚举复制值。
        self.format
    }

    // 返回 D3D11 输入布局使用的无符号字节偏移。
    pub(crate) const fn offset_bytes(self) -> u32 {
        // 偏移保持共享 ABI 原值。
        self.offset_bytes
    }

    // 返回 OpenGL 属性接口可无损接收的有符号字节偏移。
    pub(crate) const fn offset_bytes_i32(self) -> Option<i32> {
        // 超过 GLint 上限的偏移不能进入共同值域。
        if self.offset_bytes > i32::MAX as u32 {
            // 使用空值拒绝符号截断。
            return None;
        }
        // 已验证值允许无损转换。
        Some(self.offset_bytes as i32)
    }
}

// 定义 position float2 布局的唯一属性序列。
const POSITION_F32X2_ATTRIBUTES: [PipelineVertexAttribute; 1] = [PipelineVertexAttribute::new(
    // position 固定绑定到 shader location 0。
    0,
    // 该输入使用位置语义。
    PipelineVertexSemantic::Position,
    // position 包含两个浮点分量。
    PipelineVertexFormat::Float32x2,
    // position 从顶点起点开始。
    0,
)];

// 定义 position float2 与 coverage float1 的唯一属性序列。
const POSITION_COVERAGE_F32_ATTRIBUTES: [PipelineVertexAttribute; 2] = [
    PipelineVertexAttribute::new(
        0,
        PipelineVertexSemantic::Position,
        PipelineVertexFormat::Float32x2,
        0,
    ),
    PipelineVertexAttribute::new(
        1,
        PipelineVertexSemantic::Coverage,
        PipelineVertexFormat::Float32,
        8,
    ),
];

// 定义 position 与 uv 各一个 float2 的唯一属性序列。
const POSITION_UV_F32_ATTRIBUTES: [PipelineVertexAttribute; 2] = [
    // 目标位置固定绑定到 location 0。
    PipelineVertexAttribute::new(
        0,
        PipelineVertexSemantic::Position,
        PipelineVertexFormat::Float32x2,
        0,
    ),
    // 绝对 source UV 固定绑定到 location 1，紧随 position。
    PipelineVertexAttribute::new(
        1,
        PipelineVertexSemantic::TextureCoordinate,
        PipelineVertexFormat::Float32x2,
        8,
    ),
];

// 定义 position、uv 与 color 布局的唯一属性序列。
const POSITION_UV_COLOR_F32_ATTRIBUTES: [PipelineVertexAttribute; 3] = [
    // 定义顶点位置属性。
    PipelineVertexAttribute::new(
        // position 固定绑定到 shader location 0。
        0,
        // 该输入使用位置语义。
        PipelineVertexSemantic::Position,
        // position 包含两个浮点分量。
        PipelineVertexFormat::Float32x2,
        // position 从顶点起点开始。
        0,
    ),
    // 定义纹理坐标属性。
    PipelineVertexAttribute::new(
        // uv 固定绑定到 shader location 1。
        1,
        // 该输入使用纹理坐标语义。
        PipelineVertexSemantic::TextureCoordinate,
        // uv 包含两个浮点分量。
        PipelineVertexFormat::Float32x2,
        // uv 紧随八字节 position。
        8,
    ),
    // 定义逐顶点颜色属性。
    PipelineVertexAttribute::new(
        // color 固定绑定到 shader location 2。
        2,
        // 该输入使用颜色语义。
        PipelineVertexSemantic::Color,
        // color 包含四个浮点分量。
        PipelineVertexFormat::Float32x4,
        // color 紧随 position 与 uv。
        16,
    ),
];

// 定义 Adapter 必须映射的有限顶点布局。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum PipelineVertexLayout {
    // 只包含 position float2。
    PositionF32x2,
    // 包含 position float2 与 coverage float1。
    PositionCoverageF32,
    // 包含 position float2 与绝对 source uv float2。
    PositionUvF32,
    // 包含 position float2、uv float2 与 color float4。
    PositionUvColorF32,
}

// 为顶点布局集中提供完整属性序列与步长。
impl PipelineVertexLayout {
    // 返回一个顶点的固定字节数。
    pub(crate) const fn stride_bytes(self) -> u32 {
        // 逐个封闭布局返回唯一 ABI。
        match self {
            // position float2 固定占八字节。
            Self::PositionF32x2 => 8,
            // position float2 与 coverage float1 固定占十二字节。
            Self::PositionCoverageF32 => 12,
            // position 与 uv 各一个 float2，固定占十六字节。
            Self::PositionUvF32 => 16,
            // position、uv 与 color 固定占三十二字节。
            Self::PositionUvColorF32 => 32,
        }
    }

    // 返回 OpenGL 属性接口可无损接收的有符号步长。
    pub(crate) const fn stride_bytes_i32(self) -> Option<i32> {
        // 读取共享布局的唯一无符号步长。
        let stride = self.stride_bytes();
        // 超过 GLsizei 上限的步长不能进入共同值域。
        if stride > i32::MAX as u32 {
            // 使用空值拒绝符号截断。
            return None;
        }
        // 已验证值允许无损转换。
        Some(stride as i32)
    }

    // 返回两个 Adapter 必须按顺序消费的同一属性序列。
    pub(crate) const fn attributes(self) -> &'static [PipelineVertexAttribute] {
        // 逐个封闭布局返回静态共享描述。
        match self {
            // float2 布局只包含 position。
            Self::PositionF32x2 => &POSITION_F32X2_ATTRIBUTES,
            // 实心网格布局包含 position 与 coverage。
            Self::PositionCoverageF32 => &POSITION_COVERAGE_F32_ATTRIBUTES,
            // Blur 布局只包含 position 与绝对 source UV。
            Self::PositionUvF32 => &POSITION_UV_F32_ATTRIBUTES,
            // float8 布局包含 position、uv 与 color。
            Self::PositionUvColorF32 => &POSITION_UV_COLOR_F32_ATTRIBUTES,
        }
    }

    // 验证属性位置唯一且全部落在 stride 内的共同 ABI。
    pub(crate) const fn is_valid(self) -> bool {
        // 共享步长必须能被 OpenGL 有符号参数表达。
        if self.stride_bytes_i32().is_none() {
            // 无法表达的布局不得进入任何 Adapter。
            return false;
        }
        // 读取该布局的唯一属性序列。
        let attributes = self.attributes();
        // 从第一项开始验证。
        let mut index = 0;
        // 使用 const 兼容循环覆盖全部属性。
        while index < attributes.len() {
            // 复制当前属性值。
            let attribute = attributes[index];
            // 属性位置必须落在共享槽位集合内。
            if attribute.location() >= PIPELINE_VERTEX_ATTRIBUTE_SLOT_COUNT {
                // 越界位置会让 Adapter 选择不同 shader 输入。
                return false;
            }
            // 使用 checked 加法计算属性末端。
            let Some(end) = attribute
                // 从共享偏移开始。
                .offset_bytes()
                // 加上共享格式宽度。
                .checked_add(attribute.format().size_bytes())
            else {
                // 溢出属性不能进入原生 API。
                return false;
            };
            // 属性必须完整包含在一个顶点内。
            if end > self.stride_bytes() || attribute.offset_bytes_i32().is_none() {
                // 越界或无法有符号表达时拒绝。
                return false;
            }
            // 从下一项开始检查位置唯一性。
            let mut other = index + 1;
            // 比较当前项与全部后续项。
            while other < attributes.len() {
                // 重复位置会让 Adapter 覆盖同一个输入槽。
                if attribute.location() == attributes[other].location() {
                    // 共享布局必须拒绝歧义。
                    return false;
                }
                // 推进后续项。
                other += 1;
            }
            // 推进当前项。
            index += 1;
        }
        // 全部布局事实都满足共同 ABI。
        true
    }
}

// 验证两个现有顶点布局的共享属性序列。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../../tests/unit/native/present/rhi/vertex_layout__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
