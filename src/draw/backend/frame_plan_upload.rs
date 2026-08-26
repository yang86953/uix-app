//! FramePlan 拥有的类型化顶点、索引与 Uniform 上传载荷。

// 使用共享引用计数切片保存不可变顶点和索引事实，避免帧计划复制大网格。
use std::sync::Arc;

// 引入已经由共享 RHI 冻结字段语义的各类常量值对象。
use crate::platform::presentation::rhi::{
    IndexFormat, PipelineUniformLayout, PipelineVertexLayout, RhiBlurRasterParams,
    RhiGradientRasterParams, RhiMeshRasterParams, RhiMsdfRasterParams, RhiSampledRasterParams,
    RhiSectorRasterParams, RhiShadowRasterParams, RhiShapeRasterParams,
};

// 将已初始化的紧密 f32 切片借为当前 host 的 native-endian 字节视图。
fn f32_slice_as_ne_bytes(values: &[f32]) -> &[u8] {
    // SAFETY: f32 没有未初始化填充，u8 对齐为 1，返回切片不超过原借用生命周期。
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

// 将已初始化的紧密 u32 切片借为当前 host 的 native-endian 字节视图。
fn u32_slice_as_ne_bytes(values: &[u32]) -> &[u8] {
    // SAFETY: u32 没有未初始化填充，u8 对齐为 1，返回切片不超过原借用生命周期。
    unsafe {
        std::slice::from_raw_parts(values.as_ptr().cast::<u8>(), std::mem::size_of_val(values))
    }
}

// 保存 FramePlan 允许上传的封闭顶点布局与浮点值。
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FrameVertexPayload {
    // 保存只含物理位置 float2 的顶点流。
    PositionF32x2(Arc<[f32]>),
    // 保存物理位置 float2 与单位 coverage float1 的实心网格顶点流。
    PositionCoverageF32(Arc<[f32]>),
    // 保存目标 NDC position 与绝对 source UV 组成的固定 float4 顶点流。
    PositionUvF32(Arc<[f32]>),
    // 保存物理位置、UV 与颜色组成的固定 float8 顶点流。
    PositionUvColorF32(Arc<[f32]>),
}

// 为类型化顶点载荷提供构造、验证与唯一字节编码入口。
impl FrameVertexPayload {
    // 构造 position-float2 顶点载荷。
    pub(crate) fn position_f32x2(values: impl Into<Arc<[f32]>>) -> Self {
        // 保留调用方已经冻结的浮点顺序。
        Self::PositionF32x2(values.into())
    }

    // 构造 position/coverage-float3 顶点载荷。
    pub(crate) fn position_coverage_f32(values: impl Into<Arc<[f32]>>) -> Self {
        // 保留共享抗锯齿组件已经冻结的 xy 与 coverage 顺序。
        Self::PositionCoverageF32(values.into())
    }

    // 构造 position/uv-float4 顶点载荷。
    pub(crate) fn position_uv_f32(values: impl Into<Arc<[f32]>>) -> Self {
        // 保留共享 Blur 几何已经冻结的 position 与绝对 UV 顺序。
        Self::PositionUvF32(values.into())
    }

    // 构造 position/uv/color-float8 顶点载荷。
    pub(crate) fn position_uv_color_f32(values: impl Into<Arc<[f32]>>) -> Self {
        // 保留调用方已经冻结的浮点顺序。
        Self::PositionUvColorF32(values.into())
    }

    // 返回本载荷声明的唯一共享顶点布局。
    pub(crate) const fn layout(&self) -> PipelineVertexLayout {
        // 通过封闭枚举映射，禁止调用方另传可能冲突的 stride。
        match self {
            // position-float2 对应两个浮点的共享布局。
            Self::PositionF32x2(_) => PipelineVertexLayout::PositionF32x2,
            // 实心网格对应 position 与单位 coverage 的共享布局。
            Self::PositionCoverageF32(_) => PipelineVertexLayout::PositionCoverageF32,
            // position/uv-float4 对应 Blur 的共享布局。
            Self::PositionUvF32(_) => PipelineVertexLayout::PositionUvF32,
            // sampled 顶点对应八个浮点的共享布局。
            Self::PositionUvColorF32(_) => PipelineVertexLayout::PositionUvColorF32,
        }
    }

    // 借出不可变浮点序列供验证和编码共同消费。
    pub(crate) fn values(&self) -> &[f32] {
        // 两个变体都只暴露相同的只读浮点视图。
        match self {
            // 借出 position-float2 数据。
            Self::PositionF32x2(values) => values,
            // 借出 position/coverage-float3 数据。
            Self::PositionCoverageF32(values) => values,
            // 借出 position/uv-float4 数据。
            Self::PositionUvF32(values) => values,
            // 借出 sampled float8 数据。
            Self::PositionUvColorF32(values) => values,
        }
    }

    // 从唯一布局和浮点序列派生完整顶点数量。
    pub(crate) fn vertex_count(&self) -> Option<u32> {
        // 读取本布局每个顶点包含的浮点数量。
        let floats_per_vertex = self.layout().stride_bytes() as usize / std::mem::size_of::<f32>();
        // 读取唯一浮点序列。
        let values = self.values();
        // 不为残缺载荷伪造完整顶点数量。
        if values.len() % floats_per_vertex != 0 {
            // 残缺顶点没有可供 DrawRange 比较的数量。
            return None;
        }
        // 将完整顶点数量收敛到 DrawRange 的 u32 值域。
        u32::try_from(values.len() / floats_per_vertex).ok()
    }

    // 验证载荷非空、有限并包含完整顶点。
    pub(crate) fn is_valid(&self) -> bool {
        // 读取本布局每个顶点包含的浮点数量。
        let floats_per_vertex = self.layout().stride_bytes() as usize / std::mem::size_of::<f32>();
        // 读取唯一浮点序列。
        let values = self.values();
        // 空载荷、残缺顶点或非有限值都不能进入 Adapter。
        if values.is_empty()
            // 每个顶点必须严格匹配共享 stride。
            || values.len() % floats_per_vertex != 0
            // 非有限几何会让不同 rasterizer 产生未定义差异。
            || !values.iter().all(|value| value.is_finite())
        {
            // 在共享边界统一拒绝基础载荷错误。
            return false;
        }
        // 只有带颜色的 sampled 顶点需要额外的单位颜色域约束。
        match self {
            // position-float2 不携带颜色，保持原有有限值行为。
            Self::PositionF32x2(_) => true,
            // 实心网格 coverage 固定为每个 float3 顶点的第三项，且必须位于单位域。
            Self::PositionCoverageF32(_) => values
                .chunks_exact(floats_per_vertex)
                .all(|vertex| vertex[2] >= 0.0 && vertex[2] <= 1.0),
            // Blur position/uv 只要求完整且有限，UV 边界由共享几何门禁产生。
            Self::PositionUvF32(_) => true,
            // sampled float8 的每个完整顶点都必须携带单位颜色。
            Self::PositionUvColorF32(_) => values.chunks_exact(floats_per_vertex).all(
                // 逐顶点检查颜色字段而不影响位置与 UV 的值域。
                |vertex| {
                    // 颜色字段固定位于 float8 的索引 4 到 7。
                    vertex[4..8].iter().all(
                        // 共享契约只接受闭区间 [0,1] 的颜色通道。
                        |color| *color >= 0.0 && *color <= 1.0,
                    )
                },
            ),
        }
    }

    // 返回编码后的确定字节数量。
    pub(crate) fn size_bytes(&self) -> usize {
        // 浮点序列使用紧密 IEEE f32 排列。
        self.values().len() * std::mem::size_of::<f32>()
    }

    // 借出与逐值 to_ne_bytes 完全一致的 host-order 紧密字节视图。
    pub(crate) fn as_ne_bytes(&self) -> &[u8] {
        // 封闭变体都只持有紧密 f32 序列，可共享同一只读投影。
        f32_slice_as_ne_bytes(self.values())
    }

    // 在唯一 FramePlan 执行器内编码为当前 host 的紧密字节。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整顶点流预留精确容量。
        let mut bytes = Vec::with_capacity(self.size_bytes());
        // 按 Drawing 已冻结的顺序编码每个浮点。
        for value in self.values() {
            // Adapter 与执行器位于同一 host，使用 native-endian 上传。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回只在 Device 原语边界存在的字节表示。
        bytes
    }
}

// 保存 FramePlan 唯一允许的索引格式与不可变索引值。
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FrameIndexPayload {
    // 保存紧密的 host-order u32 索引序列。
    Uint32(Arc<[u32]>),
}

// 为索引载荷提供封闭构造、验证、范围查询与编码入口。
impl FrameIndexPayload {
    // 通过唯一构造器接收共享不可变的 u32 索引值。
    pub(crate) fn uint32(values: impl Into<Arc<[u32]>>) -> Self {
        // 将调用方值收回 FramePlan 的不可变所有权。
        Self::Uint32(values.into())
    }

    // 返回索引载荷的唯一共享格式。
    pub(crate) const fn format(&self) -> IndexFormat {
        // 封闭枚举禁止调用方另传格式。
        match self {
            // 当前唯一载荷变体固定为 Uint32。
            Self::Uint32(_) => IndexFormat::Uint32,
        }
    }

    // 返回只读索引值切片。
    pub(crate) fn values(&self) -> &[u32] {
        // 只投影封闭载荷内部的共享值，不暴露可变存储。
        match self {
            // 返回 u32 序列的只读视图。
            Self::Uint32(values) => values,
        }
    }

    // 验证索引载荷非空且数量可表达为共享 u32 计数。
    pub(crate) fn is_valid(&self) -> bool {
        // 与可用计数投影共享同一非空和 u32 值域门禁。
        self.index_count().is_some()
    }

    // 返回可安全传入 DrawRange 的索引数量。
    pub(crate) fn index_count(&self) -> Option<u32> {
        // 空载荷不得向 DrawRange 伪造可用的零计数。
        if self.values().is_empty() {
            // 用无结果保留索引载荷的非空不变式。
            return None;
        }
        // 索引数量通过 checked 转换收敛到 DrawRange 的 u32 值域。
        u32::try_from(self.values().len()).ok()
    }

    // 返回紧密 u32 索引序列的字节数量。
    pub(crate) fn size_bytes(&self) -> usize {
        // 每个索引固定占用一个 u32 的 native 字节宽度。
        self.values().len() * std::mem::size_of::<u32>()
    }

    // 借出与逐值 to_ne_bytes 完全一致的 host-order 紧密字节视图。
    pub(crate) fn as_ne_bytes(&self) -> &[u8] {
        // 当前闭集只允许紧密 u32 索引，不含任何结构体填充。
        u32_slice_as_ne_bytes(self.values())
    }

    // 查询 DrawRange 指定非空子范围中的最大索引值。
    pub(crate) fn max_index_in_range(&self, first: u32, count: u32) -> Option<u32> {
        // 将起始位置转换为平台切片索引并拒绝无法转换的值。
        let start = usize::try_from(first).ok()?;
        // 将范围长度转换为平台切片长度并拒绝无法转换的值。
        let length = usize::try_from(count).ok()?;
        // 使用 checked 加法计算独占末端，避免整数回绕。
        let end = start.checked_add(length)?;
        // 空范围不产生最大索引。
        if length == 0 {
            // 统一将空 DrawRange 映射为无结果。
            return None;
        }
        // 切片越界时返回无结果，合法范围再求最大值。
        self.values().get(start..end)?.iter().copied().max()
    }

    // 将索引序列编码为唯一的 native-endian 紧密字节表示。
    pub(crate) fn encode_ne_bytes(&self) -> Vec<u8> {
        // 为完整索引序列预留精确字节容量。
        let mut bytes = Vec::with_capacity(self.size_bytes());
        // 按值顺序编码每个 u32，不插入填充字节。
        for value in self.values() {
            // FramePlan 与 Device 位于同一 host，使用 native-endian 编码。
            bytes.extend_from_slice(&value.to_ne_bytes());
        }
        // 返回仅在上传边界使用的字节表示。
        bytes
    }
}

// 保存 FramePlan 允许上传的封闭 Uniform 语义值。
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum FrameUniformPayload {
    // 保存实心网格 viewport 与颜色常量。
    Mesh(RhiMeshRasterParams),
    // 保存 sampled、coverage 共用的 viewport 常量。
    Sampled(RhiSampledRasterParams),
    // 保存线性或径向渐变仿射常量。
    Gradient(RhiGradientRasterParams),
    // 保存填充或描边 Shape 常量。
    Shape(RhiShapeRasterParams),
    // 保存盒阴影仿射与软边常量。
    Shadow(RhiShadowRasterParams),
    // 保存双阶段高斯模糊常量。
    Blur(RhiBlurRasterParams),
    // 保存 MSDF viewport、atlas 与距离范围常量。
    Msdf(RhiMsdfRasterParams),
    // 保存扇形 viewport、矩形、颜色与角度常量。
    Sector(RhiSectorRasterParams),
}

// 为 Uniform 载荷提供闭集布局映射和唯一编码入口。
impl FrameUniformPayload {
    // 返回本载荷唯一对应的共享 Uniform 布局。
    pub(crate) const fn layout(self) -> PipelineUniformLayout {
        // 通过枚举变体完成穷尽映射，新增语义时由编译器要求更新。
        match self {
            // Mesh 值只能进入 Mesh 常量布局。
            Self::Mesh(_) => PipelineUniformLayout::Mesh,
            // Sampled 值只能进入 Sampled 常量布局。
            Self::Sampled(_) => PipelineUniformLayout::Sampled,
            // Gradient 值只能进入 Gradient 常量布局。
            Self::Gradient(_) => PipelineUniformLayout::Gradient,
            // Shape 值只能进入 Shape 常量布局。
            Self::Shape(_) => PipelineUniformLayout::Shape,
            // Shadow 值只能进入 Shadow 常量布局。
            Self::Shadow(_) => PipelineUniformLayout::Shadow,
            // Blur 值只能进入 Blur 常量布局。
            Self::Blur(_) => PipelineUniformLayout::Blur,
            // MSDF 值只能进入 MSDF 常量布局。
            Self::Msdf(_) => PipelineUniformLayout::Msdf,
            // Sector 值只能进入 Sector 常量布局。
            Self::Sector(_) => PipelineUniformLayout::Sector,
        }
    }

    // 验证 Uniform 载荷属于共享 FramePlan 值域。
    pub(crate) fn is_valid(self) -> bool {
        // 每个变体直接委托其共享值对象的冻结字段门禁。
        match self {
            // 检查 Mesh 全部字段。
            Self::Mesh(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Sampled 全部字段。
            Self::Sampled(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // Gradient 由共享值对象同时验证有限值、模式与径向半径。
            Self::Gradient(value) => value.is_valid(),
            // 检查 Shape 全部字段。
            Self::Shape(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Shadow 全部字段。
            Self::Shadow(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Blur 全部字段。
            Self::Blur(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 MSDF 全部字段。
            Self::Msdf(value) => value.as_f32s().iter().all(|field| field.is_finite()),
            // 检查 Sector 全部字段。
            Self::Sector(value) => value.as_f32s().iter().all(|field| field.is_finite()),
        }
    }

    // 返回共享 PipelineContract 声明的固定字节数量。
    pub(crate) const fn size_bytes(self) -> usize {
        // 统一从布局契约读取，禁止枚举另存一套大小表。
        self.layout().size_bytes()
    }

    // 借出固定 Uniform 值对象的 host-order 紧密字节视图。
    pub(crate) fn as_ne_bytes(&self) -> &[u8] {
        // 每个共享值对象都以私有 f32 数组唯一拥有 ABI 字段顺序。
        match self {
            Self::Mesh(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Sampled(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Gradient(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Shape(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Shadow(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Blur(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Msdf(value) => f32_slice_as_ne_bytes(value.as_f32s()),
            Self::Sector(value) => f32_slice_as_ne_bytes(value.as_f32s()),
        }
    }

    // 在唯一 FramePlan 执行器内编码为当前 host 的紧密字节。
    pub(crate) fn encode_ne_bytes(self) -> Vec<u8> {
        // 每个变体只调用其共享值对象拥有的唯一编码器。
        match self {
            // 编码 Mesh 固定 ABI。
            Self::Mesh(value) => value.encode_ne_bytes(),
            // 编码 Sampled 固定 ABI。
            Self::Sampled(value) => value.encode_ne_bytes(),
            // 编码 Gradient 固定 ABI。
            Self::Gradient(value) => value.encode_ne_bytes(),
            // 编码 Shape 固定 ABI。
            Self::Shape(value) => value.encode_ne_bytes(),
            // 编码 Shadow 固定 ABI。
            Self::Shadow(value) => value.encode_ne_bytes(),
            // 编码 Blur 固定 ABI。
            Self::Blur(value) => value.encode_ne_bytes(),
            // 编码 MSDF 固定 ABI。
            Self::Msdf(value) => value.encode_ne_bytes(),
            // 编码 Sector 固定 ABI。
            Self::Sector(value) => value.encode_ne_bytes(),
        }
    }
}

// 验证 FramePlan 类型化上传不会退回裸字节或错误布局。
#[cfg(test)]
// 将测试实现统一存放在根 tests 目录。
#[path = "../../../tests/unit/draw/backend/frame_plan_upload__tests.rs"]
// 保留原测试模块层级与私有契约访问能力。
mod tests;
